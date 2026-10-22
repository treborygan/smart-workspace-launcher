# Extension SDK v1

SW-021 extensions are WebAssembly modules packaged in a `.swext` ZIP. This interface is experimental and opt-in. A package is installed disabled, and a user must grant requested capabilities and explicitly enable it before running it. Import preview never instantiates or executes module code.

## Package format

The ZIP must contain exactly two files at its root: `manifest.json` and `module.wasm`. Directories, duplicate names, additional entries, symlinks, absolute paths and nested paths are rejected. Limits are 2.1 MiB compressed package, 4 KiB manifest and 2 MiB Wasm module. Preview returns an opaque, random, in-memory token bound to the exact package path and full package digest. Tokens expire after five minutes, are single use, and at most 32 are retained; expired tokens are discarded and the earliest-expiring token is evicted when the cap is reached. App restart invalidates all pending previews. Install atomically consumes the token, rereads the package, checks the same path and digest, and stores the validated module in the local SQLite registry. An installed extension ID cannot be replaced in place; remove it first, which also discards its grants. Profile archives do not contain extension modules or permission grants.

The manifest is validated by [`extension_manifest.schema.json`](../contracts/extension_manifest.schema.json), the Rust IPC boundary, and package SHA-256 checks. SDK version is exactly `1.0.0`. Module registration does not execute module start code.

## Wasm interface

The module's import section must be empty: function, table, memory and global imports are all rejected. It therefore cannot depend on WASI. The host supplies no linker functions. Modules with a Wasm start section are rejected during binary preflight, before instantiation, so no start code runs. The module must export exactly the required ABI exports below; unknown exports are rejected:

- `memory`: one linear memory, at most two 64 KiB pages.
- `alloc(i32) -> i32`: reserves a writable range of the requested input size and returns its byte offset.
- `handle(i32, i32) -> i64`: receives input offset/length and returns a packed output offset/length (`offset` in high 32 bits, `length` in low 32 bits).

Before validation, the host applies structural limits independent of the Wasm file-size and execution-fuel limits: at most 512 types and functions, 64 parameters and 64 results per function type, 256 globals, 1,024 locals per function and 16,384 locals total, 100,000 operators, 64 data segments, 64 element segments, 16 standard sections, and 8 custom sections containing at most 64 KiB of custom-section payload in total. Defined tables are rejected. wasmparser validates and inspects the ABI under these limits; it does not compile or instantiate the module. Wasmi compiles the module once only after the user explicitly selects Run on an enabled extension. The host uses wasmi with a 1,000,000 fuel budget, one memory limited to 128 KiB, recursion depth limited to 128 and value stack limited to 64 KiB. Input and output JSON are each limited to 64 KiB. The host checks pointer arithmetic and memory bounds before writes/reads, decodes strict UTF-8 JSON, and validates the response. Traps, out-of-fuel, malformed data and invalid references become ordinary command failures.

## Invocation data

Input is validated against [`extension_invocation.schema.json`](../contracts/extension_invocation.schema.json). It has `sdkVersion: "1.0.0"`, the exact granted capability names, and only the data allowed by those grants. The schema requires each granted capability's data field and forbids that field when the capability is absent:

- `resource_metadata.read`: up to 200 active-profile summaries with resource UUID, display name, type, category, tags and favorite state. Target locations, URLs, arguments and file contents are never included.
- `workspace_summary.read`: aggregate active-profile resource and favorite counts only.

Unrequested or ungranted data fields are omitted. Metadata summaries are ordered by resource UUID ascending and capped at 200 entries. The backend appends summaries in that order only while the complete serialized invocation remains within 64 KiB; it omits the first and later entries that do not fit. `truncated` is true if either the 200-entry cap or byte budget omits any profile resource. A module has no host calls, filesystem, network, process, clock, random, Shell, launch or secret access. The contract does not include extension storage, background execution or automatic activation.

## Output data

Output schema is `{ "suggestions": [{ "title": string, "reason": string, "resourceIds": string[] }] }`. At most 20 suggestions are accepted; titles are 1–80 characters, reasons 1–160, and each suggestion may reference at most 12 unique resources already assigned to the selected profile. Unknown fields, controls, invalid UUIDs, oversized strings and references outside the profile are rejected. The app renders suggestions as escaped text only. They do not modify the registry, profile or resources and cannot trigger launches. Results are held only in the current UI and are not recorded as launch history or telemetry.
