use std::sync::Arc;

use console::style;

use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use dialoguer::Editor;
use dialoguer::FuzzySelect;
use dialoguer::Input;

use crate::config::ConfigProperties;
use crate::error;
use crate::remote::Member;
use crate::remote::MergeRequestBodyArgs;
use crate::Result;

pub struct MergeRequestUserInput {
    pub title: String,
    pub description: String,
    pub user_id: i64,
    pub username: String,
}

impl MergeRequestUserInput {
    pub fn new(title: &str, description: &str, user_id: i64, username: &str) -> Self {
        MergeRequestUserInput {
            title: title.to_string(),
            description: description.to_string(),
            user_id,
            username: username.to_string(),
        }
    }
}

/// Given a new merge request, prompt user for assignee, title and description.
pub fn prompt_user_merge_request_info(
    default_title: &str,
    default_description: &str,
    members: &[Member],
    config: Arc<impl ConfigProperties>,
) -> Result<MergeRequestUserInput> {
    let (title, description) = prompt_user_title_description(default_title, default_description);

    let mut usernames = members
        .iter()
        .map(|member| &member.username)
        .collect::<Vec<&String>>();

    // Set the configuration preferred assignee username at the top of the
    // list. This way, we will just quickly enter (accept) the default value
    // without having to type to fuzzy search the one we want.

    let preferred_assignee_username = config.preferred_assignee_username();
    let preferred_assignee_username_index = {
        || -> usize {
            for (index, member) in usernames.iter().enumerate() {
                if *member == preferred_assignee_username {
                    return index;
                }
            }
            0
        }
    }();
    let preferred_member = usernames.remove(preferred_assignee_username_index);
    usernames.insert(0, preferred_member);

    let assignee_selection_id = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Assignee:")
        .default(0)
        .items(&usernames)
        .interact()
        .unwrap();

    let assignee_members_index = if assignee_selection_id != 0 {
        // Inserted in 0 the preferred one. All shifted by 1 in usernames
        // vec, so we need to shift back the index for members.
        if assignee_selection_id <= preferred_assignee_username_index {
            assignee_selection_id - 1
        } else {
            assignee_selection_id
        }
    } else {
        // The preferred one has been selected
        preferred_assignee_username_index
    };

    Ok(MergeRequestUserInput::new(
        &title,
        &description,
        members[assignee_members_index].id,
        &members[assignee_members_index].username,
    ))
}

pub fn prompt_user_title_description(
    default_title: &str,
    default_description: &str,
) -> (String, String) {
    let title: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Title: ")
        .default(default_title.to_string())
        .interact_text()
        .unwrap();

    let description = get_description(default_description);
    (title, description)
}

fn get_description(default_description: &str) -> String {
    show_input("Description: ", default_description, true, Style::Bold);
    let mut description = default_description.to_string();
    let prompt = "Edit description";
    while !confirm(prompt, false) {
        description = if let Some(entry_msg) = Editor::new().edit(&description).unwrap() {
            entry_msg
        } else {
            "".to_string()
        };
        show_input("Description: ", &description, true, Style::Bold);
    }
    description
}

pub enum Style {
    Bold,
    Light,
}

pub fn show_input(prompt: &str, data: &str, new_line: bool, font_style: Style) {
    let mut prompt_style = style(prompt);
    if let Style::Bold = font_style {
        prompt_style = prompt_style.bold()
    }
    if new_line {
        println!("{}", prompt_style);
        println!("\n{}\n", data);
    } else {
        print!("{}: ", prompt_style);
        println!("{}", data)
    }
}

fn confirm(prompt: &str, default_answer: bool) -> bool {
    if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(default_answer)
        .interact()
        .unwrap()
    {
        return default_answer;
    }
    !default_answer
}

pub fn show_summary_merge_request(
    commit_str: &str,
    args: &MergeRequestBodyArgs,
    accept: bool,
) -> Result<()> {
    show_input(
        "\nSummary of outgoing changes:",
        commit_str,
        true,
        Style::Bold,
    );
    show_input("Target branch", &args.target_branch, false, Style::Bold);
    show_input("Assignee", &args.username, false, Style::Bold);
    show_input("Title", &args.title, false, Style::Bold);
    if !args.description.is_empty() {
        show_input("Description:", &args.description, true, Style::Bold);
    } else {
        show_input("Description", "None", false, Style::Bold);
    }
    println!();
    if accept || confirm("Confirm summary", true) {
        Ok(())
    } else {
        Err(error::gen("User cancelled"))
    }
}
