use std::sync::Arc;

use console::style;

use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use dialoguer::Editor;
use dialoguer::FuzzySelect;
use dialoguer::Input;

use crate::cmds::merge_request::MergeRequestBodyArgs;
use crate::cmds::project::Member;
use crate::config::ConfigProperties;
use crate::error;
use crate::Result;

#[derive(Builder)]
pub struct MergeRequestUserInput {
    pub title: String,
    pub description: String,
    pub assignee: Member,
    #[builder(default)]
    pub reviewer: Member,
}

impl MergeRequestUserInput {
    pub fn builder() -> MergeRequestUserInputBuilder {
        MergeRequestUserInputBuilder::default()
    }

    pub fn new(title: &str, description: &str, user_id: i64, username: &str) -> Self {
        MergeRequestUserInput {
            title: title.to_string(),
            description: description.to_string(),
            assignee: Member::builder()
                .id(user_id)
                .username(username.to_string())
                .build()
                .unwrap(),
            reviewer: Member::default(),
        }
    }
}

/// Given a new merge request, prompt user for assignee, title and description.
pub fn prompt_user_merge_request_info(
    default_title: &str,
    default_description: &str,
    config: &Arc<dyn ConfigProperties>,
) -> Result<MergeRequestUserInput> {
    let (title, description) = prompt_user_title_description(default_title, default_description);

    let mut members = config.merge_request_members();
    let default_assignee = config.preferred_assignee_username();
    let mut default_assigned = false;
    if let Some(assignee) = default_assignee {
        members.insert(0, assignee);
        default_assigned = true;
        // Allow client to unselect the default assignee, leaving it blank.
        members.insert(1, Member::default());
    } else {
        // Default to blank - not assigned
        members.insert(0, Member::default());
    }

    let assignee_members_index = gather_member(&members, "Assignee:");
    let assigned_member = members[assignee_members_index].clone();

    // Reviewer selection:
    // Remove the default assignee from the list of reviewers, leaving it blank
    // as default in first position.
    if default_assigned {
        members.remove(0);
    }
    let reviewer_members_index = gather_member(&members, "Reviewer:");

    Ok(MergeRequestUserInput::builder()
        .title(title)
        .description(description)
        .assignee(assigned_member)
        .reviewer(members[reviewer_members_index].clone())
        .build()
        .unwrap())
}

fn gather_member(members: &[Member], prompt: &str) -> usize {
    let usernames = members
        .iter()
        .map(|member| member.username.as_str())
        .collect::<Vec<&str>>();

    let assignee_selection_id = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(0)
        .items(&usernames)
        .interact()
        .unwrap();

    if assignee_selection_id != 0 {
        assignee_selection_id
    } else {
        // The preferred one has been selected
        0
    }
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
    show_input("Assignee", &args.assignee.username, false, Style::Bold);
    show_input("Reviewer", &args.reviewer.username, false, Style::Bold);
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

pub fn prompt_args() -> String {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("args: ")
        .allow_empty(true)
        .interact_text()
        .unwrap()
}

pub fn fuzzy_select(amps: Vec<String>) -> Result<String> {
    let selection = dialoguer::FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("amp:")
        .default(0)
        .items(&amps)
        .interact()
        .unwrap();
    Ok(amps[selection].to_string())
}
