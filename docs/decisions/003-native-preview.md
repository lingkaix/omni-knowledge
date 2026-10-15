# Native Controls And Confined Preview

Date: 2026-10-14.
Status: Implementation decision under the engineer's explicit GPUI selection.
Owner: [Product requirements](../specs/product.md).

## Decision

Use GPUI for native controls and an embedded WKWebView for rendered documents and media.
Bundle Mermaid and highlight.js so preview does not require a remote content service.
Sanitize imported Markdown and HTML; only trusted bundled renderer scripts may execute.
Serve original media through a confined custom protocol with bounded range responses.
Reject path traversal, arbitrary embedded navigation, and resources outside the selected library.
Route an explicit source-link click to a document anchor, a contained library source, or the system browser for HTTP and HTTPS links.
Reject unsupported schemes and credentials; a source link cannot invoke file mutations or installation.
Retain native application fallback for PDF, Office, and unsupported codecs.
Use GPUI runtime shaders because the host lacks the optional offline Metal Toolchain.
Assemble a fresh local app bundle, sign its nested CLI, then apply an ad-hoc signature to the complete app.
Require deep, strict signature verification before replacing the previous artifact.
This local packaging method does not authorize Developer ID signing, notarization, or publication.
Keep user guidance and explicit Homebrew installation controls in a separate native guide page.
Keep document preview content unable to invoke installation or mutate files.
Homebrew install buttons hand off fixed package commands to a visible Terminal window.
Terminal owns the installation interface and lifetime; the app reports handoff and later tool discovery.
Keep retrieval controls in compact top rows, folder management in an on-demand panel, and files in a collapsible directory tree.
Use the mark as the return-to-workspace control and make selected-file metadata editing contextual.
Use GPUI Kit 0.5.1 controls and InputState because they share GPUI 0.2.2 with the existing runtime-shader setup.
Delegate native editing, input-method composition, selection, and clipboard shortcuts to the maintained input component.
When a native input receives focus or a pointer click, return the Cocoa first responder from WebKit to GPUI.
GPUI's logical focus alone does not restore the native responder after a preview click.
Set the single-line input's actual style height to the shared 28-pixel control height; the component's convenience height setter applies to multiline inputs.
Run preview selection, search, and explicit indexing independently, and coalesce rapid selections so stale completions cannot replace the chosen file.
Give each opened library a resource-origin token and bind document bases, media URLs, and navigation to it; obsolete pages cannot resolve the next library's same-named source.
Read selected-file facts from live attributes and authored metadata instead of the full vector manifest.
Save metadata without starting indexing; this keeps a small authored edit separate from model work and derived publication.
Update native preview bounds and visibility only when they change.
Show contextual hints and operation feedback inside the bottom status bar's layout because a native child WebKit view can cover GPUI overlays above it.
Do not reserve separate instructional or transfer-destination strips under the tree.
Treat pointer interaction with the native preview as an outside click for file context menus while preserving the preview's own interaction.
Opening a file menu returns Cocoa focus to GPUI; the existing foreground tick dismisses it when this preview's native view hierarchy receives focus.
Identify each menu session so delayed dismissal from a replaced menu cannot close the new target's menu.
This bridge uses AppKit view identity and adds no document script or source-content privilege.
Use a virtualized tree with live directory contents so empty folders and unindexed files remain manageable.
Keep search as a clearable filter over this browsing tree.
Offer native Quick Look and default-application actions for formats such as RAF that the embedded image renderer does not support.

## Alternatives And Consequences

Implementing HTML, Mermaid, PDF, and media rendering directly in GPUI would duplicate mature rendering engines.
A browser-only interface would not meet the requested native GPUI choice.
A hosted preview service would add privacy and deployment effects without serving the local requirement.
WKWebView adds platform integration work and needs actual native-window verification.
Linker-signed executables do not supply the resource seal and bound plist required by the assembled app.
The 2026-10-14 user launch failure established that bundle verification and a fresh LaunchServices launch are required observations.
Homebrew creates its own process groups and sessions, so app-owned group cancellation cannot establish whole-install cleanup.
Visible Terminal handoff preserves interactive installation without adding process-tree management to the knowledge app.
Permanent folder and metadata inputs consume reading space although folder changes are infrequent and metadata edits need a selection.
The engineer reported inconsistent focus, Chinese composition, selection, and word-editing shortcuts in the manual input implementation.
Native input components centralize this behavior and require actual macOS editing checks beyond a successful build.
Full snapshot reads on each file selection scale with the library's vectors rather than the selected source.
A global busy flag couples preview, metadata, and index latency and prevents ordinary reading during long model work.
Extracted Office and EPUB content is useful for a first version but does not preserve every layout feature.
Univer's Office import/export licensing and application footprint do not justify embedding it for this first local preview.
See [the extraction research](../dependencies-extraction.md) for the investigated alternatives.

## Revisit Conditions

Improve native rich-document layout when user testing identifies a concrete reading task blocked by extraction preview.
Revisit the renderer boundary if embedded content needs new privileges; do not silently permit source scripts or remote requests.
