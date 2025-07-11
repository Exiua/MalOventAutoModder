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
enum HScene {
    None,
    Immediate,
    Later,
}

#[derive(Debug)]
struct ChoiceDetails {
    h_scene: HScene,
    malo_rep_change: Option<RepChange>,
    bad_ending: Option<String>,
    ending: Option<String>,
    route_divergence: Option<String>,
}

impl ChoiceDetails {
    fn new(h_scene: HScene, malo_rep_change: Option<RepChange>, bad_ending: Option<String>, ending: Option<String>, route_divergence: Option<String>) -> Self {
        ChoiceDetails {
            h_scene,
            malo_rep_change,
            bad_ending,
            ending,
            route_divergence,
        }
    }
}

pub fn mod_file(file_path: &str) -> Result<(), ModderError> {
    let contents = std::fs::read_to_string(file_path)?;
    let lines = contents.lines().collect::<Vec<&str>>();
    let mut modified_lines = lines.iter().map(|s| s.to_string()).collect::<Vec<String>>();
    let mut menu_spaces = None; // If None, not in a menu block
    let mut named_malo = false;
    // Only care about modifying menu item lines
    for (i, line) in lines.iter().enumerate() {
        let cleaned_line = line.trim();
        // Once MalO is named, we can use that name when formatting choices
        if cleaned_line.starts_with("persistent.malOname = ") {
            named_malo = true;
            continue;
        }

        // Since Ren'py is Python-based, indentation delimits scope.
        let leading_spaces = count_leading_spaces(line);
        if let Some(spaces) = menu_spaces {
            // If indentation of current line is less than or equal to the menu block's indentation
            if leading_spaces <= spaces {
                menu_spaces = None; // Exiting menu block
                continue;
            }

            // Heuristic for detecting menu items
            if cleaned_line.ends_with("\":") {
                println!("Found menu item: {}", cleaned_line);
                let route_divergence = get_route_divergence(lines[i - 1]); // Route divergence is usually a comment on the line before the menu item
                if let Some(divergence) = route_divergence.as_ref() {
                    println!("Leads to {}", divergence);
                }
                
                // Read the rest of the choice block to get additional details
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
    let ChoiceDetails { h_scene, malo_rep_change, bad_ending, ending, route_divergence } = choice_details;
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

    match h_scene {
        HScene::None => (),
        HScene::Immediate => choice = format!("{} [pink]Sex Scene", choice),
        HScene::Later => choice = format!("{} [pink]Sex Scene Later", choice),
    }

    if let Some(ending) = bad_ending {
        choice = format!("{} [red]{}", choice, ending);
    }

    let mut route_set = false;
    if let Some(route) = route_divergence {
        choice = format!("{} [blue]{}", choice, route);
        route_set = true;
    }

    if !route_set {
        if let Some(route) = route_name {
            choice = format!("{} [blue]{}", choice, route);
        }
    }

    if let Some(ending) = ending {
        choice = format!("{} [gold]{}", choice, ending);
    }

    format!("{}{}\":", " ".repeat(indent_size), choice)
}

fn get_details(lines: &[&str], indent_size: usize) -> ChoiceDetails {
    // The dev usually keeps relevant variables and comments immediately after the start of the menu item
    let mut in_variables = true;
    // Indicates if the choice leads to an H-Scene
    let mut h_scene = HScene::None;
    // Holds the change in player's reputation with MalO
    let mut malo_rep_change = None;
    // Holds the bad ending, if any
    let mut bad_ending = None;
    // Holds the proper ending, if any
    let mut ending = None;
    // Holds the possible route changes, if any
    // Used to override the route determined through the preceding comment
    let mut route_override = None;
    for line in lines {
        let leading_spaces = count_leading_spaces(line);
        let cleaned_line = line.trim();
        // Don't want to break parsing if the line is empty (will cause leading_spaces to be 0)
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
                    h_scene = HScene::Immediate;
                }
                // Some of these endings can actually branch to different endings based on player choices
                // But we don't have access to player data, so we can only guess
                else if cleaned_line.contains("a3_si_zellen = True") {
                    ending = Some("Caged Ending".to_string());
                }
                else if cleaned_line.contains("a3_helped_zellen = False") {
                    // Forgot what was supposed to go here
                }
                else if cleaned_line.contains("a3_GTA_MalOd = True") {
                    route_override = Some("Possible Bad Ending".to_string());
                }
                else if cleaned_line.contains("label bye_bye_MalO:") {
                    ending = Some("True Ending".to_string());
                }
                else if cleaned_line.contains("a3_lied_zellen = True") {
                    // Not sure how to do this as it can lead to an h-scene later if malO is uncaged
                    // But can also not lead to an h-scene at all
                    //h_scene = HScene::Later;
                }
            }
            else {
                in_variables = false;
            }
        }
        // Some H-Scenes may either set the _h variable or check if hscene_on is set, so both checks are needed
        else if cleaned_line.contains("persistent.hscene_on") {
            h_scene = HScene::Immediate;
        }
        // Jumps may indicate an ending
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

    match h_scene {
        HScene::None => (),
        _ => println!("Choice leads to H-Scene"),
    }

    ChoiceDetails::new(h_scene, malo_rep_change, bad_ending, ending, route_override)
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
    // A lot of the naming is kept from the original mod this was based on
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