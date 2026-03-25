use crate::applescript::run_applescript;
use crate::models::{Area, CreateTask, Project, Tag, Task, UpdateTask};

fn things_auth_token() -> String {
    std::env::var("THINGS_AUTH_TOKEN").unwrap_or_default()
}

fn things_url_update(id_expr: &str, params: &str) -> String {
    let token = things_auth_token();
    let auth = if token.is_empty() {
        String::new()
    } else {
        format!("&auth-token={token}")
    };
    format!("set tid to id of {id_expr}\ndo shell script \"open -g 'things:///update?id=\" & tid & \"{params}{auth}'\"")
}

/// Escape a string for safe embedding in AppleScript double-quoted strings.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Parse `missing value` returns from AppleScript into Option<String>.
fn parse_optional(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() || s == "missing value" {
        None
    } else {
        Some(s.to_string())
    }
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

pub fn get_tasks(
    list_filter: Option<&str>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<Task>, String> {
    let list_spec = match list_filter {
        None | Some("inbox") | Some("") => "list \"Inbox\"".to_string(),
        Some("today") => "list \"Today\"".to_string(),
        Some("upcoming") => "list \"Upcoming\"".to_string(),
        Some("anytime") => "list \"Anytime\"".to_string(),
        Some("someday") => "list \"Someday\"".to_string(),
        Some("logbook") => "list \"Logbook\"".to_string(),
        Some("trash") => "list \"Trash\"".to_string(),
        Some(other) => format!("list \"{}\"", esc(other)),
    };

    let offset_val = offset.unwrap_or(0);
    let limit_val = limit.unwrap_or(50);
    let start = offset_val + 1; // AppleScript is 1-indexed
    let end = offset_val + limit_val;

    // Each task is serialised as a pipe-delimited record:
    // id|title|notes|due_date|project_title|area_title|tags|completed|canceled|creation_date|completion_date
    let script = format!(
        r#"tell application "Things3"
    set output to ""
    set theTasks to to dos of {list_spec}
    set taskCount to count of theTasks
    set startIdx to {start}
    set endIdx to {end}
    if endIdx > taskCount then set endIdx to taskCount
    if startIdx > taskCount then return ""
    repeat with i from startIdx to endIdx
        set t to item i of theTasks
        set tid to id of t
        set ttitle to name of t
        set tnotes to notes of t
        if tnotes is missing value then set tnotes to ""
        set tdue to due date of t
        if tdue is missing value then
            set tdue to ""
        else
            set tdue to (tdue as string)
        end if
        set tproject to ""
        if project of t is not missing value then set tproject to name of project of t
        set tarea to ""
        if area of t is not missing value then set tarea to name of area of t
        set ttags to ""
        set tagList to tags of t
        repeat with tg in tagList
            if ttags is "" then
                set ttags to name of tg
            else
                set ttags to ttags & "," & name of tg
            end if
        end repeat
        set tcompleted to (status of t is completed)
        set tcanceled to (status of t is canceled)
        set tcreation to creation date of t
        if tcreation is missing value then
            set tcreation to ""
        else
            set tcreation to (tcreation as string)
        end if
        set tcompletion to completion date of t
        if tcompletion is missing value then
            set tcompletion to ""
        else
            set tcompletion to (tcompletion as string)
        end if
        set output to output & tid & "␞" & ttitle & "␞" & tnotes & "␞" & tdue & "␞" & tproject & "␞" & tarea & "␞" & ttags & "␞" & tcompleted & "␞" & tcanceled & "␞" & tcreation & "␞" & tcompletion & "␟"
    end repeat
    return output
end tell"#
    );

    let raw = run_applescript(&script)?;
    let tasks = raw
        .split('␟')
        .filter(|l| !l.trim().is_empty())
        .map(|l| parse_task_line(l.trim()))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(tasks)
}

pub fn get_task_by_id(task_id: &str) -> Result<Task, String> {
    let script = format!(
        r#"tell application "Things3"
    set t to to do id "{id}"
    set tid to id of t
    set ttitle to name of t
    set tnotes to notes of t
    if tnotes is missing value then set tnotes to ""
    set tdue to due date of t
    if tdue is missing value then
        set tdue to ""
    else
        set tdue to (tdue as string)
    end if
    set tproject to ""
    if project of t is not missing value then set tproject to name of project of t
    set tarea to ""
    if area of t is not missing value then set tarea to name of area of t
    set ttags to ""
    set tagList to tags of t
    repeat with tg in tagList
        if ttags is "" then
            set ttags to name of tg
        else
            set ttags to ttags & "," & name of tg
        end if
    end repeat
    set tcompleted to (status of t is completed)
    set tcanceled to (status of t is canceled)
    set tcreation to creation date of t
    if tcreation is missing value then
        set tcreation to ""
    else
        set tcreation to (tcreation as string)
    end if
    set tcompletion to completion date of t
    if tcompletion is missing value then
        set tcompletion to ""
    else
        set tcompletion to (tcompletion as string)
    end if
    return tid & "␞" & ttitle & "␞" & tnotes & "␞" & tdue & "␞" & tproject & "␞" & tarea & "␞" & ttags & "␞" & tcompleted & "␞" & tcanceled & "␞" & tcreation & "␞" & tcompletion
end tell"#,
        id = esc(task_id)
    );

    let raw = run_applescript(&script)?;
    parse_task_line(raw.trim())
}

fn parse_task_line(line: &str) -> Result<Task, String> {
    let parts: Vec<&str> = line.splitn(11, '␞').collect();
    if parts.len() < 11 {
        return Err(format!("Unexpected task format: {}", line));
    }

    let tags: Vec<String> = if parts[6].is_empty() {
        vec![]
    } else {
        parts[6].split(',').map(|s| s.trim().to_string()).collect()
    };

    Ok(Task {
        id: parts[0].to_string(),
        title: parts[1].to_string(),
        notes: parse_optional(parts[2]),
        due_date: parse_optional(parts[3]),
        project: parse_optional(parts[4]),
        area: parse_optional(parts[5]),
        list: None,
        tags,
        checklist_items: vec![], // fetched separately when needed
        completed: parts[7].trim() == "true",
        canceled: parts[8].trim() == "true",
        creation_date: parse_optional(parts[9]),
        completion_date: parse_optional(parts[10]),
    })
}

pub fn create_task(payload: &CreateTask) -> Result<Task, String> {
    let title = esc(&payload.title);
    let notes = payload.notes.as_deref().map(esc).unwrap_or_default();
    let due_date = payload.due_date.as_deref().unwrap_or("").to_string();

    // Build properties string
    let mut props = format!("name:\"{title}\"");
    if !notes.is_empty() {
        props.push_str(&format!(", notes:\"{notes}\""));
    }
    if !due_date.is_empty() {
        props.push_str(&format!(", due date:(date \"{due_date}\")"));
    }

    // Tags: set after creation
    let tags_script = if let Some(tags) = &payload.tags {
        if tags.is_empty() {
            String::new()
        } else {
            let tag_list = tags
                .iter()
                .map(|t| format!("\"{}\"", esc(t)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("\n    set tag names of newTask to {{{tag_list}}}")
        }
    } else {
        String::new()
    };

    // Checklist items — Things 3 doesn't support checklist manipulation via AppleScript.
    // We handle this after task creation via the URL scheme.
    let checklist_script = String::new();

    // Project assignment happens in AppleScript; list moves happen after via URL scheme
    let move_script = if let Some(project) = &payload.project {
        format!(
            "set theProject to project \"{}\"\nmove newTask to theProject",
            esc(project)
        )
    } else {
        String::new()
    };
    let list_move = match payload.list.as_deref() {
        Some("today") => Some("today"),
        Some("someday") => Some("someday"),
        Some("upcoming") => Some("upcoming"),
        Some("anytime") => Some("anytime"),
        _ => None,
    };

    let mut lines = vec![
        "tell application \"Things3\"".to_string(),
        format!("    set newTask to make new to do with properties {{{props}}}"),
    ];
    if !move_script.is_empty() {
        lines.push(format!("    {}", move_script.trim()));
    }
    if !tags_script.is_empty() {
        lines.push(format!("    {}", tags_script.trim()));
    }
    if !checklist_script.is_empty() {
        for cs_line in checklist_script.trim().lines() {
            lines.push(format!("    {}", cs_line.trim()));
        }
    }
    lines.push("    return id of newTask".to_string());
    lines.push("end tell".to_string());
    let script = lines.join("\n");

    // Debug: uncomment to see the generated script
    // eprintln!("AppleScript:\n{}", script);

    let id = run_applescript(&script)?;
    let id = id.trim().to_string();

    // Add checklist items via URL scheme (AppleScript doesn't support checklist manipulation)
    if let Some(items) = &payload.checklist_items {
        if !items.is_empty() {
            let token = things_auth_token();
            let auth = if token.is_empty() {
                String::new()
            } else {
                format!("&auth-token={token}")
            };
            let checklist_json: Vec<String> = items
                .iter()
                .map(|item| {
                    format!(
                        "{{\"type\":\"checklist-item\",\"attributes\":{{\"title\":\"{}\"}}}}",
                        esc(item)
                    )
                })
                .collect();
            let json_str = format!("[{}]", checklist_json.join(","));
            let encoded = urlencoding::encode(&json_str);
            let url = format!("things:///update?id={id}&checklist-items={encoded}{auth}");
            let open_script = format!("do shell script \"open -g '{}'\"", url.replace('\'', "'\\''"));
            let _ = run_applescript(&open_script);
            // Small delay to let Things process the URL
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    // Move to list via URL scheme (must happen after creation)
    if let Some(list) = list_move {
        let token = things_auth_token();
        let auth = if token.is_empty() {
            String::new()
        } else {
            format!("&auth-token={token}")
        };
        let url = format!("things:///update?id={id}&when={list}{auth}");
        let open_script = format!("do shell script \"open -g '{}'\"", url.replace('\'', "'\\''"));
        let _ = run_applescript(&open_script);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    get_task_by_id(&id)
}

pub fn update_task(task_id: &str, payload: &UpdateTask) -> Result<Task, String> {
    let mut updates = vec![];

    if let Some(title) = &payload.title {
        updates.push(format!("set name of t to \"{}\"", esc(title)));
    }
    if let Some(notes) = &payload.notes {
        updates.push(format!("set notes of t to \"{}\"", esc(notes)));
    }
    if let Some(due_date) = &payload.due_date {
        if due_date.is_empty() {
            updates.push("set due date of t to missing value".to_string());
        } else {
            updates.push(format!("set due date of t to (date \"{}\")", esc(due_date)));
        }
    }
    if let Some(tags) = &payload.tags {
        let tag_list = tags
            .iter()
            .map(|t| format!("\"{}\"", esc(t)))
            .collect::<Vec<_>>()
            .join(", ");
        updates.push(format!("set tag names of t to {{{tag_list}}}"));
    }
    if let Some(project) = &payload.project {
        updates.push(format!(
            "move t to project \"{}\" of application \"Things3\"",
            esc(project)
        ));
    }
    if let Some(true) = payload.completed {
        updates.push("set status of t to completed".to_string());
    }
    // List moves and uncomplete are handled after the main update via URL scheme
    let list_move = payload.list.as_deref();
    let uncomplete = matches!(payload.completed, Some(false));

    if updates.is_empty() && !uncomplete && list_move.is_none() {
        return get_task_by_id(task_id);
    }

    if !updates.is_empty() {
        let update_body = updates.join("\n    ");

        let script = format!(
            r#"tell application "Things3"
    set t to to do id "{id}"
    {update_body}
end tell"#,
            id = esc(task_id),
        );

        run_applescript(&script)?;
    }

    if uncomplete {
        let token = things_auth_token();
        let auth = if token.is_empty() {
            String::new()
        } else {
            format!("&auth-token={token}")
        };
        let url = format!("things:///update?id={}&completed=false{}", task_id, auth);
        let open_script = format!("do shell script \"open -g '{}'\"", url.replace('\'', "'\\''"));
        let _ = run_applescript(&open_script);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // Handle list move via URL scheme if requested
    if let Some(list) = list_move {
        let move_script = things_url_update(
            &format!("to do id \"{}\"", esc(task_id)),
            &format!("&when={list}"),
        );
        let move_apple = format!(
            "tell application \"Things3\"\n    {}\nend tell",
            move_script.replace('\n', "\n    ")
        );
        let _ = run_applescript(&move_apple);
    }

    get_task_by_id(task_id)
}

pub fn complete_task(task_id: &str) -> Result<Task, String> {
    let script = format!(
        r#"tell application "Things3"
    set t to to do id "{id}"
    set status of t to completed
end tell"#,
        id = esc(task_id)
    );
    run_applescript(&script)?;
    get_task_by_id(task_id)
}

pub fn delete_task(task_id: &str) -> Result<(), String> {
    let script = format!(
        r#"tell application "Things3"
    delete (to do id "{id}")
end tell"#,
        id = esc(task_id)
    );
    run_applescript(&script)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------

pub fn get_projects() -> Result<Vec<Project>, String> {
    let script = r#"tell application "Things3"
    set output to ""
    set theProjects to projects
    repeat with p in theProjects
        set pid to id of p
        set ptitle to name of p
        set pnotes to notes of p
        if pnotes is missing value then set pnotes to ""
        set parea to ""
        if area of p is not missing value then set parea to name of area of p
        set ptags to ""
        set tagList to tags of p
        repeat with tg in tagList
            if ptags is "" then
                set ptags to name of tg
            else
                set ptags to ptags & "," & name of tg
            end if
        end repeat
        set pcompleted to (status of p is completed)
        set pcanceled to (status of p is canceled)
        set output to output & pid & "␞" & ptitle & "␞" & pnotes & "␞" & parea & "␞" & ptags & "␞" & pcompleted & "␞" & pcanceled & "␟"
    end repeat
    return output
end tell"#;

    let raw = run_applescript(script)?;
    raw.split('␟')
        .filter(|l| !l.trim().is_empty())
        .map(|l| parse_project_line(l.trim()))
        .collect()
}

fn parse_project_line(line: &str) -> Result<Project, String> {
    let parts: Vec<&str> = line.splitn(7, '␞').collect();
    if parts.len() < 7 {
        return Err(format!("Unexpected project format: {}", line));
    }

    let tags: Vec<String> = if parts[4].is_empty() {
        vec![]
    } else {
        parts[4].split(',').map(|s| s.trim().to_string()).collect()
    };

    Ok(Project {
        id: parts[0].to_string(),
        title: parts[1].to_string(),
        notes: parse_optional(parts[2]),
        area: parse_optional(parts[3]),
        tags,
        completed: parts[5].trim() == "true",
        canceled: parts[6].trim() == "true",
    })
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

pub fn get_tags() -> Result<Vec<Tag>, String> {
    let script = r#"tell application "Things3"
    set output to ""
    set theTags to tags
    repeat with t in theTags
        set output to output & name of t & "\n"
    end repeat
    return output
end tell"#;

    let raw = run_applescript(script)?;
    let tags = raw
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| Tag {
            name: l.trim().to_string(),
        })
        .collect();

    Ok(tags)
}

// ---------------------------------------------------------------------------
// Areas
// ---------------------------------------------------------------------------

pub fn get_areas() -> Result<Vec<Area>, String> {
    let script = r#"tell application "Things3"
    set output to ""
    set theAreas to areas
    repeat with a in theAreas
        set aid to id of a
        set atitle to name of a
        set atags to ""
        set tagList to tags of a
        repeat with tg in tagList
            if atags is "" then
                set atags to name of tg
            else
                set atags to atags & "," & name of tg
            end if
        end repeat
        set output to output & aid & "␞" & atitle & "␞" & atags & "␟"
    end repeat
    return output
end tell"#;

    let raw = run_applescript(script)?;
    raw.split('␟')
        .filter(|l| !l.trim().is_empty())
        .map(|l| parse_area_line(l.trim()))
        .collect()
}

fn parse_area_line(line: &str) -> Result<Area, String> {
    let parts: Vec<&str> = line.splitn(3, '␞').collect();
    if parts.len() < 3 {
        return Err(format!("Unexpected area format: {}", line));
    }

    let tags: Vec<String> = if parts[2].is_empty() {
        vec![]
    } else {
        parts[2].split(',').map(|s| s.trim().to_string()).collect()
    };

    Ok(Area {
        id: parts[0].to_string(),
        title: parts[1].to_string(),
        tags,
    })
}
