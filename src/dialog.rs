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

struct MemberSelector {
    members: Vec<Member>,
}

impl MemberSelector {
    pub fn new(members: Vec<Member>) -> Self {
        Self { members }
    }

    /// Determines the assignee based on priority:
    /// 1. CLI provided assignee (if present)
    /// 2. Config preferred assignee (if present)
    /// 3. Empty list with default unassigned member
    pub fn prepare_assignee_list(
        &self,
        cli_assignee: Option<&Member>,
        config_assignee: Option<Member>,
    ) -> Vec<Member> {
        // Start with the original members list
        let mut selection_list = self.members.clone();

        match (cli_assignee, config_assignee) {
            (Some(cli), _) => {
                // CLI assignee takes precedence
                selection_list.insert(0, cli.clone());
                selection_list.insert(1, Member::default()); // Allow unassigning
            }
            (None, Some(config)) => {
                // Config assignee is secondary
                selection_list.insert(0, config);
                selection_list.insert(1, Member::default()); // Allow unassigning
            }
            (None, None) => {
                // No defaults - start with unassigned
                selection_list.insert(0, Member::default());
            }
        }

        selection_list
    }

    /// Prepares reviewer list by excluding the selected assignee
    pub fn prepare_reviewer_list(
        &self,
        default_cli_reviewer: Option<&Member>,
        assigned_member: &Member,
    ) -> Vec<Member> {
        let mut selection_list = if default_cli_reviewer.is_some() {
            vec![default_cli_reviewer.unwrap().clone(), Member::default()]
        } else {
            vec![Member::default()]
        };
        selection_list.extend(
            self.members
                .iter()
                .filter(|m| m != &assigned_member)
                .cloned(),
        );
        selection_list
    }
}

/// Given a new merge request, prompt user for assignee, title and description.
pub fn prompt_user_merge_request_info(
    default_title: &str,
    default_description: &str,
    default_cli_assignee: Option<&Member>,
    default_cli_reviewer: Option<&Member>,
    config: &Arc<dyn ConfigProperties>,
) -> Result<MergeRequestUserInput> {
    let (title, description) = prompt_user_title_description(default_title, default_description);

    // Initialize member selector with available members
    let selector = MemberSelector::new(config.merge_request_members());

    // Prepare assignee selection list with priorities
    let assignee_list =
        selector.prepare_assignee_list(default_cli_assignee, config.preferred_assignee_username());

    // Get assignee selection
    let assignee_index = gather_member(&assignee_list, "Assignee:");
    let assigned_member = assignee_list[assignee_index].clone();

    // Prepare reviewer list excluding the selected assignee
    let reviewer_list = selector.prepare_reviewer_list(default_cli_reviewer, &assigned_member);
    let reviewer_index = gather_member(&reviewer_list, "Reviewer:");

    Ok(MergeRequestUserInput::builder()
        .title(title)
        .description(description)
        .assignee(assigned_member)
        .reviewer(reviewer_list[reviewer_index].clone())
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
        println!("{prompt_style}");
        println!("\n{data}\n");
    } else {
        print!("{prompt_style}: ");
        println!("{data}")
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
    show_outgoing_changes_summary(commit_str);
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

pub fn show_outgoing_changes_summary(commit_str: &str) {
    show_input(
        "\nSummary of outgoing changes:",
        commit_str,
        true,
        Style::Bold,
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cmds::project::Member;

    fn create_test_member(id: i64, username: &str) -> Member {
        Member::builder()
            .id(id)
            .username(username.to_string())
            .build()
            .unwrap()
    }

    fn create_test_members() -> Vec<Member> {
        vec![
            create_test_member(1, "alice"),
            create_test_member(2, "bob"),
            create_test_member(3, "charlie"),
        ]
    }

    #[test]
    fn test_prepare_assignee_list_with_cli_assignee() {
        let members = create_test_members();
        let original_members = members.clone();
        let selector = MemberSelector::new(members);
        let cli_assignee = create_test_member(4, "david");

        let result = selector.prepare_assignee_list(
            Some(&cli_assignee),
            Some(create_test_member(5, "eve")), // Should be ignored when CLI assignee is present
        );

        // Verify the exact ordering
        assert_eq!(result[0], cli_assignee); // CLI assignee first
        assert_eq!(result[1], Member::default()); // Unassigned option second
        assert_eq!(&result[2..], &original_members[..]); // Original list preserved after
        assert_eq!(result.len(), original_members.len() + 2); // Original + 2 inserted items
    }

    #[test]
    fn test_prepare_assignee_list_with_config_assignee() {
        let members = create_test_members();
        let original_members = members.clone();
        let selector = MemberSelector::new(members);
        let config_assignee = create_test_member(4, "eve");

        let result = selector.prepare_assignee_list(None, Some(config_assignee.clone()));

        // Verify the exact ordering
        assert_eq!(result[0], config_assignee); // Config assignee first
        assert_eq!(result[1], Member::default()); // Unassigned option second
        assert_eq!(&result[2..], &original_members[..]); // Original list preserved after
        assert_eq!(result.len(), original_members.len() + 2); // Original + 2 inserted items
    }

    #[test]
    fn test_prepare_assignee_list_with_no_defaults() {
        let members = create_test_members();
        let original_members = members.clone();
        let selector = MemberSelector::new(members);

        let result = selector.prepare_assignee_list(None, None);

        // Verify the exact ordering
        assert_eq!(result[0], Member::default()); // Unassigned option first
        assert_eq!(&result[1..], &original_members[..]); // Original list preserved after
        assert_eq!(result.len(), original_members.len() + 1); // Original + 1 inserted item
    }

    #[test]
    fn test_prepare_assignee_list_with_existing_member() {
        let members = create_test_members();
        let original_members = members.clone();
        let selector = MemberSelector::new(members);

        // Use the first member from the list as CLI assignee
        let cli_assignee = &original_members[0];
        let result = selector.prepare_assignee_list(Some(cli_assignee), None);

        // Verify the exact ordering
        assert_eq!(result[0], cli_assignee.clone()); // CLI assignee first
        assert_eq!(result[1], Member::default()); // Unassigned option second
        assert_eq!(&result[2..], &original_members[..]); // Original list preserved after
        assert_eq!(result.len(), original_members.len() + 2); // Original + 2 inserted items
    }

    #[test]
    fn test_prepare_reviewer_list_with_cli_reviewer_provided() {
        let members = create_test_members();
        let selector = MemberSelector::new(members);
        let assignee = create_test_member(1, "alice");
        let reviewer = create_test_member(2, "charlie");

        let result = selector.prepare_reviewer_list(Some(&reviewer), &assignee);

        assert_eq!(result[0], reviewer);
        assert!(!result.contains(&assignee));
        // Reviewer (id = 2), unassigned, bob (id = 2) and charlie (id = 3)
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_prepare_reviewer_list_no_cli_reviewer_provided() {
        let members = create_test_members();
        let selector = MemberSelector::new(members);
        let assignee = create_test_member(1, "alice");

        let result = selector.prepare_reviewer_list(None::<&Member>, &assignee);

        assert_eq!(result[0], Member::default());
        assert!(!result.contains(&assignee));
        assert_eq!(result.len(), 3); // Unassigned + 2 remaining members
    }
}
