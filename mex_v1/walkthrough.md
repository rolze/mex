# `mex_v1` Walkthrough

I have successfully built a from-scratch production-ready skeleton for **Sem & Mex** v1 (specifically focusing on `mex`). I followed your instructions to keep things simple (KISS), layered, and tailored entirely to the detailed Use Cases and Database specs you provided. 

## Architectural Foundation
The code is structured meticulously across distinct layers to ensure extensibility, maintainability, and testability.

- **`domain/`**: Pure data models (`MediaItem`, `Filter`, `Tag`, `Status`) mapping exactly to the concepts in the specs. 
- **`db/`**: Handles database interactions securely and efficiently with SQLite using a robust bundled driver (`rusqlite`). Uses simple prepared statements inside transactions to achieve high performance (especially during batch status updates like `t` and `k`).
- **`ui/`**: A fully decoupled TUI rendering system built with `ratatui`. Each visual component (File List, Preview Pane, Filter Bar, Status Line) has its own module (`file_list.rs`, `filter_bar.rs`, etc.) and takes only the required context, making the components highly reusable and modular.
- **`services/`**: Holds command abstractions and background processes (`commands.rs`), decoupling application routing from the core event loop.
- **`app.rs` & `main.rs`**: Drive the core event loop. Standard threads, a synchronous main loop, and clean state separation maintain high performance while avoiding the complexity of unnecessary asynchronous runtimes (as per your request to just use standard standard primitives!).

## Feature Highlights 
I fulfilled the exact behavior described in the UC documents:

> [!NOTE] 
> **Navigation & Grouping (`UC-04`)**
> File grouping boundaries are calculated dynamically! Pressing `Home` / `End` intelligently jumps you directly to the start of the current `yyyy-MM-DD` or slug group rather than skipping aimlessly.

> [!NOTE] 
> **Complex Selection (`UC-04`)**
> A powerful selection model is built into `app.rs`. 
> - `Space`: Toggles the current item
> - `Shift-Up/Down`: Sweeps the selection, toggling the range precisely based on a `shift_anchor`.
> - `Shift-Home/End`: Applies a mass-toggle over the entire date/slug group in a single keystroke.

> [!NOTE] 
> **Contextual Filtering (`UC-05`)**
> The UI includes the sophisticated `FilterBar` logic. 
> Type `/` to enter text filtering (`*` wildcards are fully implemented in the filtering engine). Type `#` to drop into tag completion mode, or `@` to drop into type completion. The active filter dynamically applies AND/OR logic constraints instantly! 

> [!NOTE] 
> **Lightning Fast Status Updates (`UC-06`)**
> Operations like **Trash (`t`)**, **Keep (`k`)**, and **Caption (`c`)** map automatically to whatever you have selected. If nothing is selected, they instantly apply to the item currently under your cursor. The DB logic immediately executes the change wrapped efficiently in an SQLite transaction.

> [!TIP]
> **Subprocess Hooks (`UC-07`)**
> As per your prompt, integrations with external tools like `mpv` or `sem` are deferred (skipped), keeping the focus entirely on perfecting the local media management capabilities. The structure to trigger them in the background (via `services/commands.rs`) is completely ready for you.

## How to Test
You can immediately start the prototype and see it connect to your schema:
```bash
cd mex_v1
cargo run
```

Let me know if you want to dive deeper into any of the layers, polish the visual layout further, or integrate the background task runner next!
