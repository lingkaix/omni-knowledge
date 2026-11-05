//! Extent-accounted native ZIP output, including seeks, headers, and central records.
use crate::{Error, Work, charge_fresh, gate, note_written};
use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};

/// One native output with no in-memory archive and no payload-only disk estimate.
pub(crate) struct GatedWrite<'s, 'a> {
    file: File,
    work: &'s RefCell<&'s mut Work<'a>>,
    error: &'s Cell<Option<Error>>,
    position: u64,
    extent: u64,
    admitted: u64,
}

impl<'s, 'a> GatedWrite<'s, 'a> {
    /// Wrap an exclusively created empty native file.
    pub(crate) fn new(
        file: File,
        work: &'s RefCell<&'s mut Work<'a>>,
        error: &'s Cell<Option<Error>>,
    ) -> Self {
        Self {
            file,
            work,
            error,
            position: 0,
            extent: 0,
            admitted: 0,
        }
    }

    /// Return the held output after ZIP finalization.
    pub(crate) fn into_file(self) -> File {
        self.file
    }

    /// Preserve the first precise admission/cancellation error through zip's I/O facade.
    fn fail(&self, error: Error) -> io::Error {
        self.error.set(Some(error));
        io::Error::other(error)
    }
}

impl Write for GatedWrite<'_, '_> {
    /// Admit the prospective extent before growth and reconcile actual short writes.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(error) = self.error.get() {
            return Err(io::Error::other(error));
        }
        let mut work = self.work.borrow_mut();
        gate(&work).map_err(|e| self.fail(e))?;
        if bytes.is_empty() {
            return Ok(0);
        }
        let end = self
            .position
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| self.fail(Error::TooLarge))?;
        if end > work.bounds.quotas.max_archive_bytes {
            return Err(self.fail(Error::Quota));
        }
        if end > self.admitted {
            charge_fresh(&mut work, 0, end - self.admitted).map_err(|e| self.fail(e))?;
            self.admitted = end;
        }
        let count = self.file.write(bytes)?;
        self.position += count as u64;
        if count != 0 && self.position > self.extent {
            note_written(&mut work, self.position - self.extent).map_err(|e| self.fail(e))?;
            self.extent = self.position;
        }
        Ok(count)
    }

    /// Flush the native file; durability is requested by the caller after finish.
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl Seek for GatedWrite<'_, '_> {
    /// Seeking does not itself grow the file or grant allocation permission.
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        if let Some(error) = self.error.get() {
            return Err(io::Error::other(error));
        }
        gate(&self.work.borrow()).map_err(|e| self.fail(e))?;
        self.position = self.file.seek(to)?;
        Ok(self.position)
    }
}
