# things-anywhere

> **Disclaimer:** This project is not affiliated with, endorsed by, or associated with [Cultured Code](https://culturedcode.com/) or Things 3 in any way. Things 3 is a trademark of Cultured Code GmbH & Co. KG.

A REST API server for [Things 3](https://culturedcode.com/things/) (macOS task manager), written in Rust with [Axum](https://github.com/tokio-rs/axum). Bridges HTTP to Things 3 via AppleScript (`osascript`).

## Requirements

- macOS with Things 3 installed
- Rust toolchain (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Things 3 must be running when requests are made

## Setup

```bash
git clone <repo>
cd things-api
cargo build --release
```

Run the server:

```bash
# Default port 3333
THINGS_AUTH_TOKEN=your-token-here ./target/release/things-api

# Custom port
PORT=8080 THINGS_AUTH_TOKEN=your-token-here ./target/release/things-api
```

> **Note:** `THINGS_AUTH_TOKEN` is required for list assignment (moving tasks to Today, Someday, etc.). Find it in Things → Settings → General → Authentication Token.

Or in development:

```bash
cargo run
```

Enable debug logging:

```bash
RUST_LOG=debug cargo run
```

## Architecture

```
src/
├── main.rs                   # Server startup, router definition
├── models.rs                 # Shared data types (Task, Project, Tag, Area, …)
├── applescript/
│   ├── mod.rs                # osascript runner
│   └── commands.rs           # AppleScript-backed data functions
└── routes/
    ├── mod.rs
    ├── tasks.rs              # /tasks endpoints
    ├── projects.rs           # /projects endpoint
    └── tags.rs               # /tags and /areas endpoints
```

## Endpoints

### `GET /health`

Returns server status.

```bash
curl http://localhost:3333/health
```

```json
{ "status": "ok", "version": "0.1.0" }
```

---

### `GET /tasks`

Returns tasks from a Things 3 list. Defaults to **Inbox**.

| Query param | Values                                                                  |
| ----------- | ----------------------------------------------------------------------- |
| `list`      | `inbox` (default), `today`, `upcoming`, `anytime`, `someday`, `logbook` |

```bash
# Inbox (default)
curl http://localhost:3333/tasks

# Today list
curl "http://localhost:3333/tasks?list=today"

# Someday list
curl "http://localhost:3333/tasks?list=someday"
```

<details>
<summary>Example response</summary>

```json
[
  {
    "id": "ABCDEF123456",
    "title": "Buy groceries",
    "notes": "Milk, eggs, bread",
    "due_date": null,
    "list": null,
    "project": null,
    "area": "Personal",
    "tags": ["errands"],
    "checklist_items": [],
    "completed": false,
    "canceled": false,
    "creation_date": "Sunday, March 1, 2026 at 10:00:00 AM",
    "completion_date": null
  }
]
```

</details>

---

### `GET /tasks/:id`

Fetch a single task by its Things 3 ID.

```bash
curl http://localhost:3333/tasks/ABCDEF123456
```

---

### `POST /tasks`

Create a new task.

| Field             | Type     | Required | Description                                                    |
| ----------------- | -------- | -------- | -------------------------------------------------------------- |
| `title`           | string   | **yes**  | Task title                                                     |
| `notes`           | string   | no       | Body text                                                      |
| `due_date`        | string   | no       | Date string parseable by AppleScript (e.g. `"March 25, 2026"`) |
| `list`            | string   | no       | `inbox`, `today`, `upcoming`, `anytime`, `someday`             |
| `project`         | string   | no       | Exact project name (takes priority over `list`)                |
| `tags`            | string[] | no       | Tag names                                                      |
| `checklist_items` | string[] | no       | Checklist item titles                                          |

```bash
curl -X POST http://localhost:3333/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Prepare quarterly report",
    "notes": "Include Q1 metrics",
    "due_date": "March 31, 2026",
    "list": "today",
    "tags": ["work", "priority"],
    "checklist_items": ["Gather data", "Write draft", "Review"]
  }'
```

Returns `201 Created` with the created task object.

---

### `PATCH /tasks/:id`

Update task fields. All fields are optional.

| Field      | Type     | Description                                                      |
| ---------- | -------- | ---------------------------------------------------------------- |
| `title`    | string   | New title                                                        |
| `notes`    | string   | New notes                                                        |
| `due_date` | string   | New due date (empty string clears it)                            |
| `list`     | string   | Move to list: `inbox`, `today`, `upcoming`, `anytime`, `someday` |
| `tags`     | string[] | Replace tag set                                                  |
| `project`  | string   | Move to project (by name)                                        |

```bash
curl -X PATCH http://localhost:3333/tasks/ABCDEF123456 \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Updated title",
    "due_date": "April 1, 2026",
    "tags": ["work"]
  }'
```

---

### `PATCH /tasks/:id/complete`

Mark a task as completed.

```bash
curl -X PATCH http://localhost:3333/tasks/ABCDEF123456/complete
```

---

### `DELETE /tasks/:id`

Delete a task permanently.

```bash
curl -X DELETE http://localhost:3333/tasks/ABCDEF123456
```

Returns `204 No Content` on success.

---

### `GET /projects`

List all projects in Things 3.

```bash
curl http://localhost:3333/projects
```

```json
[
  {
    "id": "XYZ789",
    "title": "Home Renovation",
    "notes": "Q2 2026",
    "area": "Home",
    "tags": [],
    "completed": false,
    "canceled": false
  }
]
```

---

### `GET /tags`

List all tags.

```bash
curl http://localhost:3333/tags
```

```json
[{ "name": "work" }, { "name": "errands" }]
```

---

### `GET /areas`

List all areas.

```bash
curl http://localhost:3333/areas
```

```json
[
  {
    "id": "AREA123",
    "title": "Personal",
    "tags": []
  }
]
```

---

## Error responses

All errors return a JSON body:

```json
{ "error": "AppleScript error: ..." }
```

| Status | Meaning                              |
| ------ | ------------------------------------ |
| `400`  | Bad request / missing required field |
| `404`  | Task or resource not found           |
| `500`  | AppleScript / Things 3 error         |

## Notes

- Things 3 must be **open** for AppleScript calls to succeed.
- The `due_date` field accepts date strings as AppleScript parses them; `"March 25, 2026"` is the safest format.
- Task IDs are assigned by Things 3 and are stable for the lifetime of the task.
- The `checklist_items` array in GET responses is currently returned empty; fetching per-item details via AppleScript is expensive at list scale and can be added as needed.

## License

MIT — see [LICENSE](LICENSE).
