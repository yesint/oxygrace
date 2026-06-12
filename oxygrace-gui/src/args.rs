//! Command-line arguments, following xmgrace's conventions:
//! `oxygrace-gui project.agr`, `-xy data`, `-nxy data`, `-type TYPE data`,
//! `-free`. Unknown `-options` are reported and skipped.

use std::path::PathBuf;

use oxygrace::model::SetType;
use oxygrace::Project;

pub struct Launch {
    pub project: Option<Project>,
    /// Path of a loaded *project* file (data-only launches save via Save As).
    pub path: Option<PathBuf>,
    pub free_aspect: bool,
    pub messages: Vec<String>,
}

pub fn parse(args: impl Iterator<Item = String>) -> Launch {
    let mut launch = Launch {
        project: None,
        path: None,
        free_aspect: false,
        messages: Vec::new(),
    };
    let mut set_type = SetType::Xy;
    let mut args = args.peekable();

    let load_data = |launch: &mut Launch, file: &str, set_type: SetType, nxy: bool| {
        match std::fs::read_to_string(file) {
            Ok(content) => {
                let project = launch.project.get_or_insert_with(Project::default);
                let n = oxygrace::import::import_data_str(project, &content, set_type, nxy, 0);
                oxygrace::import::autoscale_world(project.graph_mut(0));
                launch.messages.push(format!("{file}: {n} set(s)"));
            }
            Err(e) => launch.messages.push(format!("{file}: {e}")),
        }
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-free" => launch.free_aspect = true,
            "-xy" => {
                if let Some(f) = args.next() {
                    load_data(&mut launch, &f, SetType::Xy, false);
                }
            }
            "-nxy" => {
                if let Some(f) = args.next() {
                    load_data(&mut launch, &f, SetType::Xy, true);
                }
            }
            "-type" | "-settype" => {
                if let Some(t) = args.next() {
                    match SetType::parse(&t) {
                        Some(t) => set_type = t,
                        None => launch.messages.push(format!("unknown set type {t}")),
                    }
                }
            }
            opt if opt.starts_with('-') => {
                launch.messages.push(format!("ignored option {opt}"));
            }
            file => {
                let is_project = std::path::Path::new(file)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("agr"));
                if is_project {
                    match oxygrace::load(file) {
                        Ok(p) => {
                            launch.project = Some(p);
                            launch.path = Some(PathBuf::from(file));
                        }
                        Err(e) => launch.messages.push(format!("{file}: {e}")),
                    }
                } else {
                    load_data(&mut launch, file, set_type, false);
                }
            }
        }
    }
    launch
}
