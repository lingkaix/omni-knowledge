//! Human and agent command-line entry point with structured output and explicit failures.
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use knowledge_daemon::UnboundHandler;
use omni_knowledge::{IndexOptions, KnowledgeBase, embedding, epub, extract};
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Local knowledge-library command line; mutations never delete original files.
#[derive(Parser)]
#[command(
    name = "omni",
    version,
    about = "Local knowledge search for humans and agents"
)]
struct Arguments {
    /// Selected library or any folder inside it.
    #[arg(long, short, global = true, default_value = ".")]
    root: PathBuf,
    /// Emit structured JSON, including errors on stderr.
    #[arg(long, global = true)]
    json: bool,
    /// Operation to perform.
    #[command(subcommand)]
    command: Operation,
}

/// Explicit lifecycle, retrieval, and conversion operations.
#[derive(Subcommand)]
enum Operation {
    /// Initialize a folder with an agent guide, OKF concepts, and local model configuration.
    Init {
        /// Folder to initialize; defaults to --root.
        path: Option<PathBuf>,
        /// Library display name.
        #[arg(long, default_value = "My Knowledge")]
        name: String,
    },
    /// Copy sources into raw/ without overwriting and reconcile the lexical index.
    Add {
        /// Files to copy; originals remain intact.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Exact tags for each imported source.
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// Reconcile files and publish an atomic lexical or multimodal index.
    Index {
        /// Generate vectors using the configured local provider.
        #[arg(long)]
        embeddings: bool,
        /// Re-extract and re-embed unchanged files.
        #[arg(long)]
        force: bool,
        /// Reconcile all nested libraries independently.
        #[arg(long)]
        recursive: bool,
    },
    /// Retrieve source-linked evidence with exact optional tag filters.
    Search {
        /// Query text or grep regular expression.
        #[arg(default_value = "")]
        query: String,
        /// Lexical, grep, semantic, hybrid, or filename retrieval.
        #[arg(long, value_enum, default_value = "bm25")]
        mode: QueryMode,
        /// Required complete tag, ignoring case by default; repeat for AND filtering.
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Maximum returned results.
        #[arg(long, short = 'n', default_value_t = 30)]
        limit: usize,
        /// Case-sensitive grep, filename matching, and exact tag filters.
        #[arg(long)]
        case_sensitive: bool,
        /// Image query for semantic or hybrid retrieval.
        #[arg(long)]
        image: Option<PathBuf>,
        /// Emit the complete labelled native Query result, including coverage and evidence.
        #[arg(long)]
        canonical: bool,
        /// Search independently indexed nested libraries and include root identity.
        #[arg(long)]
        recursive: bool,
    },
    /// List all indexed files, including unknown binary types.
    List,
    /// Inspect index coverage and recorded warnings without contacting the model.
    Status,
    /// Read or replace editable file tags and descriptions; reindex to publish changes.
    Metadata {
        /// Root-relative owned file path.
        path: String,
        /// Replacement exact tags; --clear-tags removes all editable tags.
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Remove editable tags (front matter tags remain governed by the document).
        #[arg(long, conflicts_with = "tags")]
        clear_tags: bool,
        /// Replacement description; omit to preserve it.
        #[arg(long)]
        description: Option<String>,
    },
    /// Generate a sanitized HTML preview from an owned document.
    Preview {
        /// Root-relative source path.
        path: String,
        /// Save HTML without overwriting, or print to stdout.
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Extract an owned source to a separate Markdown file without overwriting.
    Convert {
        /// Root-relative source path.
        path: String,
        /// Destination Markdown file.
        #[arg(long, short)]
        output: PathBuf,
    },
    /// Export Markdown and HTML chapters as an EPUB3 archive without overwriting.
    ExportEpub {
        /// Destination EPUB archive.
        output: PathBuf,
        /// Optional title; defaults to the library name.
        #[arg(long)]
        title: Option<String>,
        /// Include contained relative image, audio, and video assets where supported.
        #[arg(long)]
        include_media: bool,
    },
    /// Inspect required local executables and optional embedding readiness.
    Doctor {
        /// Contact the configured local embedding provider and validate identity.
        #[arg(long)]
        embeddings: bool,
    },
    /// Serve agent tools over newline-delimited MCP JSON-RPC on stdin/stdout.
    ServeMcp,
    /// Serve the frozen command codec once a native handler is bound.
    Serve {
        /// Serve MCP 2025-11-25 on stdin and stdout.
        #[arg(long)]
        stdio: bool,
        /// Select nested local libraries before accepting agent requests.
        #[arg(long)]
        recursive: bool,
        /// Serve opt-in loopback HTTP.
        #[arg(long)]
        http: bool,
    },
    /// Proxy MCP stdio to a loopback daemon IPC address.
    Mcp {
        /// Read MCP from stdin and write MCP to stdout.
        #[arg(long)]
        stdio: bool,
        /// `127.0.0.1:port` of a daemon IPC listener. The address is not a grant.
        #[arg(long)]
        daemon: Option<String>,
    },
}

/// Existing CLI aliases and the six frozen native methods; hybrid selects native fusion.
#[derive(Clone, Copy, ValueEnum)]
enum QueryMode {
    Bm25,
    Literal,
    Regex,
    Grep,
    Files,
    Tags,
    Semantic,
    Hybrid,
}
impl QueryMode {
    fn name(self) -> &'static str {
        match self {
            Self::Bm25 => "bm25",
            Self::Literal => "literal",
            Self::Regex | Self::Grep => "regex",
            Self::Files => "files",
            Self::Tags => "tags",
            Self::Semantic => "semantic",
            Self::Hybrid => "hybrid",
        }
    }
}

/// Runs one operation and writes machine-readable errors without hiding failure exits.
fn main() -> ExitCode {
    let arguments = Arguments::parse();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let unbound = error.downcast_ref::<UnboundHandler>().is_some();
            if arguments.json {
                eprintln!("{}", serde_json::json!({"error":format!("{error:#}")}));
            } else if unbound {
                eprintln!("{}", omni_knowledge::status::format_process_account(None));
                eprintln!("native authority handler is not bound");
            } else {
                eprintln!("Error: {error:#}");
            }
            // Drop the actual error before process termination. Its owned M3
            // PendingProcessing must finish foreground collection with all FDs,
            // source/stage objects and credit, including error-output failure.
            ExitCode::from(if unbound { 3 } else { 1 })
        }
    }
}

/// Executes the requested operation and returns errors to the process boundary.
fn run(arguments: &Arguments) -> Result<()> {
    if let Operation::Init { path, name } = &arguments.command {
        let kb = KnowledgeBase::init(path.as_deref().unwrap_or(&arguments.root), name)?;
        let report = kb.index(&IndexOptions::default())?;
        return print(
            &serde_json::json!({"root":kb.root,"index":report}),
            arguments.json,
        );
    }
    if matches!(arguments.command, Operation::ServeMcp) {
        return omni_knowledge::mcp::serve(&arguments.root);
    }
    if let Operation::Serve {
        stdio,
        http,
        recursive,
    } = &arguments.command
    {
        if !stdio && !http {
            bail!("choose serve --stdio or serve --http");
        }
        if *stdio && !*http {
            return omni_knowledge::mcp::serve_canonical(&arguments.root, *recursive);
        }
        return Err(UnboundHandler.into());
    }
    if let Operation::Mcp { stdio, daemon } = &arguments.command {
        if !stdio {
            bail!("mcp requires --stdio");
        }
        let Some(daemon) = daemon else {
            return omni_knowledge::mcp::serve_canonical(&arguments.root, false);
        };
        let address: std::net::SocketAddr = daemon
            .parse()
            .context("daemon address must be 127.0.0.1:port")?;
        if !address.ip().is_loopback() {
            bail!("daemon address must be loopback");
        }
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        return knowledge_daemon::proxy_mcp(&mut input, &mut output, address).map_err(Into::into);
    }
    if let Operation::Doctor { embeddings: false } = arguments.command {
        let runtime = omni_knowledge::runtime::detect_runtime();
        return print(
            &serde_json::json!({"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"ffmpeg":runtime.ffmpeg.is_some(),"ffprobe":runtime.ffprobe.is_some(),"runtime":runtime,"quick_look":Path::new("/usr/bin/qlmanage").exists(),"textutil":Path::new("/usr/bin/textutil").exists(),"ocr":"optional; no automatic OCR","hint":"omni doctor --embeddings --root LIBRARY validates the local model"}),
            arguments.json,
        );
    }
    let kb = KnowledgeBase::open(&arguments.root)?;
    anyhow::ensure!(
        kb.config.version == 2
            || matches!(
                arguments.command,
                Operation::List
                    | Operation::Status
                    | Operation::Preview { .. }
                    | Operation::Search { .. }
            ),
        "version-1 libraries require legacy migration (not yet shipped); use list for inventory or preview for inspection"
    );
    match &arguments.command {
        Operation::Add { files, tags } => {
            let mut paths = vec![];
            for source in files {
                paths.push(kb.import(source, tags)?);
            }
            let report = kb.index(&IndexOptions::default())?;
            print(
                &serde_json::json!({"imported":paths,"index":report}),
                arguments.json,
            )
        }
        Operation::Index {
            embeddings,
            force,
            recursive,
        } => {
            let mut libraries = vec![kb.clone()];
            if *recursive {
                libraries.extend(kb.children()?);
            }
            let mut reports = vec![];
            for library in libraries {
                reports.push(serde_json::json!({"root":library.root,"report":library.index(&IndexOptions {embeddings:*embeddings,force:*force})?}));
            }
            print(&reports, arguments.json)
        }
        Operation::Search {
            query,
            mode,
            tags,
            limit,
            case_sensitive,
            image,
            recursive,
            canonical,
        } => {
            let mut libraries = vec![kb.clone()];
            if *recursive {
                libraries.extend(kb.children()?);
            }
            let host = omni_knowledge::mcp_native::NativeQueryHost::open(libraries, true)?;
            let request = omni_knowledge::mcp_native::legacy_request(
                &host,
                query,
                mode.name(),
                tags,
                *limit,
                *case_sensitive,
                image.is_some(),
            )?;
            let mut output = std::io::stdout().lock();
            host.with_query(&request, |reply| {
                if *canonical && !reply.is_error() {
                    reply.write_search(&mut output)?;
                } else if arguments.json {
                    reply.write_legacy(&mut output)?;
                } else {
                    reply.write_text(&mut output)?;
                }
                output.write_all(b"\n")?;
                output.flush()?;
                anyhow::ensure!(!reply.is_error(), "native query refused");
                Ok(())
            })
        }
        Operation::List => print(
            &kb.list()?
                .iter()
                .map(omni_knowledge::FileRecord::metadata_summary)
                .collect::<Vec<_>>(),
            arguments.json,
        ),
        Operation::Status => print(&kb.status()?, arguments.json),
        Operation::Metadata {
            path,
            tags,
            clear_tags,
            description,
        } => {
            if *clear_tags || !tags.is_empty() || description.is_some() {
                let current = kb.metadata(path)?;
                let replacement = if *clear_tags {
                    vec![]
                } else if tags.is_empty() {
                    current.tags
                } else {
                    tags.clone()
                };
                kb.set_metadata(path, &replacement, description.as_deref())?;
            }
            print(&kb.metadata(path)?, arguments.json)
        }
        Operation::Preview { path, output } => {
            let html = extract::preview_html(&kb.resolve(path)?, omni_knowledge::MAX_TEXT_BYTES)?;
            if let Some(destination) = output {
                write_new(destination, &html)?;
                print(&serde_json::json!({"output":destination}), arguments.json)
            } else if arguments.json {
                print(&serde_json::json!({"html":html}), true)
            } else {
                println!("{html}");
                Ok(())
            }
        }
        Operation::Convert { path, output } => {
            let extracted = extract::extract(&kb.resolve(path)?, omni_knowledge::MAX_TEXT_BYTES)?;
            if extracted.text.is_empty() {
                bail!(
                    "no text extracted; source retained: {}",
                    extracted.warnings.join("; ")
                );
            }
            write_new(output, &extracted.text)?;
            print(
                &serde_json::json!({"output":output,"status":extracted.status,"warnings":extracted.warnings}),
                arguments.json,
            )
        }
        Operation::ExportEpub {
            output,
            title,
            include_media,
        } => {
            let count = epub::export_epub(
                &kb.root,
                output,
                title.as_deref().unwrap_or(&kb.config.name),
                *include_media,
            )?;
            print(
                &serde_json::json!({"output":output,"chapters":count}),
                arguments.json,
            )
        }
        Operation::Doctor { embeddings: true } => {
            let config = kb
                .config
                .embedding
                .as_ref()
                .context("embedding provider not configured")?;
            print(
                &embedding::EmbeddingClient::new(config)?.health()?,
                arguments.json,
            )
        }
        Operation::Init { .. }
        | Operation::ServeMcp
        | Operation::Serve { .. }
        | Operation::Mcp { .. }
        | Operation::Doctor { embeddings: false } => unreachable!(),
    }
}

/// Prints serializable values as structured JSON or readable indented JSON.
fn print<T: Serialize>(value: &T, compact: bool) -> Result<()> {
    if compact {
        println!("{}", serde_json::to_string(value)?);
    } else {
        println!("{}", serde_json::to_string_pretty(value)?);
    }
    Ok(())
}

/// Creates a user-requested output atomically without replacing an existing file.
fn write_new(path: &Path, text: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .context("output exists or cannot be created")?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(())
}
