use once_cell::sync::Lazy;
use regex::Regex;
use thiserror::Error;

//const INDENT_SIZE: usize = 4; // Number of spaces for each indent level

static H_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"a[0-9]_(?:malo|si|zellen)_h[0-9]").unwrap()
});

#[derive(Error, Debug)]
pub enum ModderError {
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug)]
enum RepChange {
    Increase(String),
    Decrease(String),
}

#[derive(Debug)]
enum Ending {
    BadEnding(String),
    GoodEnding(String),
}

#[derive(Debug)]
struct ChoiceDetails {
    h_scene: bool,
    malo_rep_change: Option<RepChange>,
    bad_ending: Option<String>,
    ending: Option<String>,
}

impl ChoiceDetails {
    fn new(h_scene: bool, malo_rep_change: Option<RepChange>, bad_ending: Option<String>, ending: Option<String>) -> Self {
        ChoiceDetails {
            h_scene,
            malo_rep_change,
            bad_ending,
            ending,
        }
    }
}

pub fn mod_file(file_path: &str) -> Result<(), ModderError> {
    let contents = std::fs::read_to_string(file_path)?;
    let lines = contents.lines().collect::<Vec<&str>>();
    let mut modified_lines = lines.iter().map(|s| s.to_string()).collect::<Vec<String>>();
    let mut menu_spaces = None; // If None, not in a menu block
    let mut named_malo = false;
    for (i, line) in lines.iter().enumerate() {
        let cleaned_line = line.trim();
        if cleaned_line.starts_with("persistent.malOname = ") {
            named_malo = true;
            continue;
        }

        let leading_spaces = count_leading_spaces(line);
        if let Some(spaces) = menu_spaces {
            if leading_spaces <= spaces {
                menu_spaces = None; // Exiting menu block
                continue;
            }

            if cleaned_line.ends_with("\":") {
                println!("Found menu item: {}", cleaned_line);
                let route_divergence = get_route_divergence(lines[i - 1]); // Route divergence is usually a comment on the line before the menu item
                if let Some(divergence) = route_divergence.as_ref() {
                    println!("Leads to {}", divergence);
                }
                
                let choice_details = get_details(&lines[i + 1..], leading_spaces);
                let formatted_choice = format_choice(cleaned_line, choice_details, named_malo, route_divergence, leading_spaces);
                println!("Formatted choice: {}", formatted_choice);
                modified_lines[i] = formatted_choice;
            }
        }
        else if cleaned_line.starts_with("menu:") {
            menu_spaces = Some(leading_spaces);
            continue;
        }
    }

    let backup_path = format!("{}.bak", file_path);
    std::fs::copy(file_path, &backup_path)?;
    println!("Backup created at: {}", backup_path);
    let modified_content = modified_lines.join("\n") + "\n"; // Ensure the file ends with a newline
    std::fs::write(file_path, modified_content)?;

    Ok(())
}

fn format_choice(choice: &str, choice_details: ChoiceDetails, named_malo: bool, route_name: Option<String>, indent_size: usize) -> String {
    let mut choice = choice.strip_suffix("\":").unwrap().to_string();
    let ChoiceDetails { h_scene, malo_rep_change, bad_ending, ending } = choice_details;
    if let Some(rep_change) = malo_rep_change {
        match rep_change {
            RepChange::Increase(value) => {
                choice = if named_malo {
                    format!("{} [gr]([malOname] +{})", choice, value)
                }
                else {
                    format!("{} [gr](MalO +{})", choice, value)
                }
            },
            RepChange::Decrease(value) => {
                choice = if named_malo {
                    format!("{} [red]([malOname] -{})", choice, value)
                }
                else {
                    format!("{} [red](MalO -{})", choice, value)
                }
            },
        }
    }

    if h_scene {
        choice = format!("{} [pink]Sex Scene", choice);
    }

    if let Some(ending) = bad_ending {
        choice = format!("{} [red]{}", choice, ending);
    }

    if let Some(route) = route_name {
        choice = format!("{} [blue]{}", choice, route);
    }

    if let Some(ending) = ending {
        choice = format!("{} [gold]{}", choice, ending);
    }

    format!("{}{}\":", " ".repeat(indent_size), choice)
}

fn get_details(lines: &[&str], indent_size: usize) -> ChoiceDetails {
    let mut in_variables = true;
    let mut h_scene = false;
    let mut malo_rep_change = None;
    let mut bad_ending = None;
    let mut ending = None;
    for line in lines {
        let leading_spaces = count_leading_spaces(line);
        let cleaned_line = line.trim();
        if leading_spaces <= indent_size && !cleaned_line.is_empty() {
            break; // Exit if we are no longer in the same block
        }

        if in_variables {
            if cleaned_line.starts_with("#") {
                if cleaned_line.contains("Ending") {
                    bad_ending = Some(cleaned_line.replace("#", "").trim().to_string());
                }
                else if cleaned_line.contains("bad end") || cleaned_line.contains("Bad End"){
                    bad_ending = Some("Bad Ending".to_string());
                }
            }
            else if cleaned_line.starts_with("$") {
                if cleaned_line.contains("MalO_Rep") {
                    if cleaned_line.contains("+=") {
                        let rep_increase = cleaned_line.split("=").nth(1).unwrap_or("").trim();
                        println!("MalO Rep increase: {}", rep_increase);
                        malo_rep_change = Some(RepChange::Increase(rep_increase.to_string()));
                    }
                    else if cleaned_line.contains("-=") {
                        let rep_decrease = cleaned_line.split("=").nth(1).unwrap_or("").trim();
                        println!("MalO Rep decrease: {}", rep_decrease);
                        malo_rep_change = Some(RepChange::Decrease(rep_decrease.to_string()));
                    }
                }
                else if H_PATTERN.is_match(cleaned_line) {
                    h_scene = true;
                }
                else if cleaned_line.contains("a3_si_zellen = True") {
                    ending = Some("Caged Ending".to_string());
                }
                else if cleaned_line.contains("a3_helped_zellen = False") {

                }
            }
            else {
                in_variables = false;
            }
        }
        else if cleaned_line.contains("persistent.hscene_on") {
            h_scene = true;
        }
        else if cleaned_line.starts_with("jump") {
            let end = get_ending(cleaned_line);
            if let Some(end) = end {
                match end {
                    Ending::BadEnding(end) => bad_ending = Some(end),
                    Ending::GoodEnding(end) => ending = Some(end),
                }
            }
        }
    }

    if h_scene {
        println!("Choice leads to H-Scene");
    }

    ChoiceDetails::new(h_scene, malo_rep_change, bad_ending, ending)
}

fn get_ending(line: &str) -> Option<Ending> {
    let line = line.trim();
    if line.contains("Act_2_Coomer_End") {
        return Some(Ending::BadEnding("Ending 3?".to_string()));
    }
    else if line.contains("bye_bye_MalO") {
        return Some(Ending::GoodEnding("True Ending".to_string()));
    }
    else if line.contains("good_end") {
        return Some(Ending::GoodEnding("Good Ending".to_string()));
    }

    None
}

fn get_route_divergence(line: &str) -> Option<String> {
    let line = line.trim();
    let route_name = if line.starts_with("#") {
        let normalized_line = line.to_ascii_lowercase();
        if normalized_line.contains("route") || normalized_line.contains("track") {
            line.strip_prefix("#").map(|s| s.trim().to_string())
        }
        else {
            None
        }
    }
    else{
        None
    };

    if let Some(route_name) = route_name {
        normalize_route_name(&route_name)
    }
    else{
        None
    }
}

fn normalize_route_name(route_name: &str) -> Option<String> {
    match route_name {
        "Coomer/Waifu Route" => Some("Coomer/Waifu Route".to_string()),
        "Friendly Route" => Some("Friendly Route".to_string()),
        "Ignore Route" => Some("end?".to_string()),
        "The Friendly, non-advancing route" => Some("The Friendly, non-advancing route".to_string()),
        "The Si-Won divergence from the Coom track" => Some("Divergence from the Coomer track".to_string()),
        "Locks you into the coomer route for real. No going back!" => Some("Locks you into the coomer route".to_string()),
        _ => None
    }
}

fn count_leading_spaces(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

#[test]
fn test_mod_file() {
    let file_path = r"";
    mod_file(file_path).expect("Failed to modify file");
}