use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
    ops::Index,
};

use crate::{error::GRError, Result};

/// A .gitlab-ci.yml is a sequence of stages, where each stage is a collection
/// of jobs. A stage name is unique, so we can uniquely identify them by name.
#[derive(Debug)]
pub struct Stage {
    pub name: String,
    pub jobs: Vec<Job>,
}

impl Stage {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            jobs: vec![],
        }
    }
}

type StageName = String;

// Map of stage names to their respective stages.
pub struct StageMap {
    stage_names: Vec<StageName>,
    stages: HashMap<StageName, Stage>,
}

impl StageMap {
    fn new() -> Self {
        Self {
            stage_names: vec![],
            stages: HashMap::new(),
        }
    }

    fn insert(&mut self, name: StageName, stage: Stage) {
        self.stage_names.push(name.clone());
        self.stages.insert(name, stage);
    }

    fn get_mut(&mut self, name: &str) -> Option<&mut Stage> {
        self.stages.get_mut(name)
    }

    fn contains_key(&self, name: &str) -> bool {
        self.stages.contains_key(name)
    }
}

/// A job is a unique unit of work that is executed in a gitlab-ci pipeline. They
/// belong to a stage. No job can be named the same, so we can uniquely identify
/// them by name.
#[derive(Debug)]
pub struct Job {
    pub name: String,
    pub rules: Vec<HashMap<String, CicdEntity>>,
}

impl Job {
    pub fn new(name: &str, rules: Vec<HashMap<String, CicdEntity>>) -> Self {
        Self {
            name: name.to_string(),
            rules,
        }
    }
}

/// Defines a CicdEntity entity that can be a sequence, a mapping, a string or null.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CicdEntity {
    Vec(Vec<CicdEntity>),
    Hash(HashMap<String, CicdEntity>),
    String(String),
    Integer(i64),
    Null,
}

impl CicdEntity {
    pub fn as_vec(&self) -> Option<&Vec<CicdEntity>> {
        if let CicdEntity::Vec(ref v) = *self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_hash(&self) -> Option<&HashMap<String, CicdEntity>> {
        if let CicdEntity::Hash(ref h) = *self {
            Some(h)
        } else {
            None
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let CicdEntity::String(ref s) = *self {
            Some(s)
        } else {
            None
        }
    }
}

// Index operations to have similar ergonomics as Yaml.
impl Index<&str> for CicdEntity {
    type Output = CicdEntity;

    fn index(&self, index: &str) -> &Self::Output {
        if let CicdEntity::Hash(ref h) = *self {
            h.get(index).unwrap_or(&CicdEntity::Null)
        } else {
            &CicdEntity::Null
        }
    }
}

impl Index<usize> for CicdEntity {
    type Output = CicdEntity;

    fn index(&self, index: usize) -> &Self::Output {
        if let CicdEntity::Vec(ref v) = *self {
            v.get(index).unwrap_or(&CicdEntity::Null)
        } else {
            &CicdEntity::Null
        }
    }
}

pub enum EntityName {
    Stage,
    Job,
}

impl AsRef<str> for EntityName {
    fn as_ref(&self) -> &str {
        match self {
            EntityName::Stage => "stages",
            EntityName::Job => "jobs",
        }
    }
}

pub trait ToCicdEntity {
    fn get(&self, entity_name: &Option<EntityName>) -> CicdEntity;
}

pub trait CicdParser {
    fn get_stages(&self) -> Result<StageMap>;
    /// Gathers the jobs and populate the stages with their corresponding jobs
    fn get_jobs(&self, stages: &mut StageMap);
}

// Encapsulates the YAML parser library that we use to parse the YAML file.
pub struct YamlParser<T> {
    parser: T,
}

impl<T> YamlParser<T> {
    pub fn new(parser: T) -> Self {
        Self { parser }
    }
}

impl<T: ToCicdEntity> CicdParser for YamlParser<T> {
    fn get_stages(&self) -> Result<StageMap> {
        let entity = self.parser.get(&Some(EntityName::Stage));
        if let Some(cicd_stage_names) = entity.as_vec() {
            let mut stages = StageMap::new();
            for cicd_stage_name in cicd_stage_names {
                if let Some(stage_name) = cicd_stage_name.as_str() {
                    let stage = Stage::new(stage_name);
                    stages.insert(stage_name.to_string(), stage);
                }
            }
            Ok(stages)
        } else {
            Err(GRError::MermaidParsingError("No stages found".to_string()).into())
        }
    }

    fn get_jobs(&self, stages: &mut StageMap) {
        let entity = self.parser.get(&Some(EntityName::Job));
        if let Some(cicd_job_details) = entity.as_hash() {
            for (job, job_details) in cicd_job_details {
                let job_name = job.as_str();
                //verify if the job has a stage
                let stage = job_details["stage"].as_str();
                if stage.is_none() {
                    // could be an anchor `.template` without an associated stage
                    continue;
                }
                let stage = stage.unwrap();
                // All jobs need a corresponding stage. If the stage does not
                // exit for this job, then technically is a wrong configuration.
                // We skip it as it's not a valid job that can be added to a
                // stage. User will get an error message when pushing project to
                // GitLab or when linting the file.
                if !stages.contains_key(stage) {
                    // could be an anchor `.template` without an associated stage
                    continue;
                }
                let mut rules: Vec<HashMap<String, CicdEntity>> = job_details["rules"]
                    .as_vec()
                    .map(|rules| {
                        rules
                            .iter()
                            .map(|rule| {
                                if let Some(rule) = rule.as_hash() {
                                    let mut rule_map = HashMap::new();
                                    for (key, value) in rule.iter() {
                                        rule_map.insert(key.clone(), value.clone());
                                    }
                                    rule_map
                                } else if let Some(rule) = rule.as_vec() {
                                    let mut rule_map = HashMap::new();
                                    for rule in rule {
                                        if let Some(rule) = rule.as_hash() {
                                            for (key, value) in rule {
                                                let value = value.clone();
                                                rule_map.insert(key.clone(), value);
                                            }
                                        }
                                    }
                                    rule_map
                                } else {
                                    // empty rules
                                    HashMap::new()
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                // if job_name has white spaces join them with a hyphen
                let job_name = job_name.split_whitespace().collect::<Vec<&str>>().join("-");
                // if rules is empty, check only rules
                let only = job_details["only"].as_vec();
                if only.is_some() {
                    rules = vec![];
                    for rule in only.unwrap() {
                        let mut rule_map = HashMap::new();
                        rule_map.insert("only".to_string(), rule.clone());
                        rules.push(rule_map);
                    }
                } else {
                    let refs = job_details["only"]["refs"].as_vec();
                    if refs.is_some() {
                        rules = vec![];
                        for rule in refs.unwrap() {
                            let mut rule_map = HashMap::new();
                            rule_map.insert("only".to_string(), rule.clone());
                            rules.push(rule_map);
                        }
                    }
                }
                // If job begins with dot, then it's a template.
                if job_name.starts_with('.') {
                    continue;
                }
                let job = Job::new(&job_name, rules.clone());
                let mut parallel_jobs = vec![];
                // check if it's a parallel job
                if let Some(parallel) = job_details["parallel"].as_hash() {
                    let matrix = parallel.get(&"matrix".to_string()).unwrap();
                    let matrix = matrix.as_vec().unwrap();
                    let mut all_values = vec![];
                    for matrix_item in matrix {
                        let partial_values = combine_matrix_values(matrix_item);
                        all_values.push(partial_values);
                    }
                    for val_matrix in all_values {
                        for val in val_matrix {
                            parallel_jobs
                                .push(Job::new(&format!("{}-{}", job_name, val), rules.clone()))
                        }
                    }
                }
                if parallel_jobs.is_empty() {
                    stages.get_mut(stage).unwrap().jobs.push(job);
                } else {
                    for parallel_job in parallel_jobs {
                        stages.get_mut(stage).unwrap().jobs.push(parallel_job);
                    }
                }
            }
        }
    }
}

fn combine_matrix_values(matrix: &CicdEntity) -> Vec<String> {
    let map = matrix.as_hash().unwrap();
    let keys = map.keys().collect::<Vec<&String>>();
    let mut all_values = vec![];
    let mut previous_values = vec![];
    let num_matrix_keys = keys.len();
    for key in keys {
        let values = if let Some(values) = map.get(key).unwrap().as_vec() {
            values
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect::<Vec<String>>()
        } else {
            vec![map.get(key).unwrap().as_str().unwrap().to_string()]
        };
        let mut new_values = vec![];
        for value in values.iter() {
            if previous_values.is_empty() {
                new_values.push(value.to_string());
            } else {
                for previous_value in previous_values.iter() {
                    new_values.push(format!("{}-{}", previous_value, value.as_str()));
                    all_values.push(format!("{}-{}", previous_value, value.as_str()));
                }
            }
        }
        previous_values = new_values;
    }
    if all_values.is_empty() && num_matrix_keys == 1 {
        // This is the case where there is only one key in the matrix. The
        // values could be an array, so we need one job per value.
        all_values = previous_values;
    }
    all_values
}

#[derive(Default)]
pub struct Mermaid {
    pub buf: Vec<String>,
}

impl Mermaid {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, line: String) {
        self.buf.push(line);
    }
}

impl Display for Mermaid {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for line in self.buf.iter() {
            writeln!(f, "{}", line)?;
        }
        Ok(())
    }
}

/// Generate a Mermaid state diagram with each stage encapsulating all its jobs
/// and the links in between stages.
pub fn generate_mermaid_stages_diagram(parser: impl CicdParser) -> Result<Mermaid> {
    let mut mermaid = Mermaid::new();
    mermaid.push("stateDiagram-v2".to_string());
    mermaid.push("    direction LR".to_string());

    let mut stages = parser.get_stages()?;

    parser.get_jobs(&mut stages);

    for (i, stage) in stages.stage_names.iter().enumerate() {
        let stage_obj = stages.stages.get(stage).unwrap();
        let jobs = &stage_obj.jobs;

        // Replace - for _ in stage name to avoid mermaid errors
        let stage_name = stage_obj.name.replace('-', "_");

        // Include .pre and .post stages only if they have jobs
        if (stage_name == ".pre" || stage_name == ".post") && jobs.is_empty() {
            continue;
        }

        mermaid.push(format!("    state {}{}", stage_name, "{"));
        let anchor_name = format!("anchorT{}", i);
        mermaid.push("        direction LR".to_string());
        mermaid.push(format!("        state \"jobs\" as {}", anchor_name));
        for job in jobs.iter() {
            mermaid.push(format!("        state \"{}\" as {}", job.name, anchor_name));
        }
        mermaid.push(format!("    {}", "}"));

        // check all next stages for compatibility. If the first stage after
        // current one is compatible and the second stage after current one is
        // also compatible, there should not be a link between the first and the
        // second.
        'stages: for next_stage_name in stages.stage_names.iter().skip(i + 1) {
            let next_stage_obj = stages.stages.get(next_stage_name).unwrap();
            let next_jobs = &next_stage_obj.jobs;

            // Skip .pre and .post stages if they have no jobs
            if (next_stage_obj.name == ".pre" || next_stage_obj.name == ".post")
                && next_jobs.is_empty()
            {
                continue;
            }

            // Replace - for _ in stage name to avoid mermaid errors
            let next_stage_name = next_stage_obj.name.replace('-', "_");

            // if there's compatibility after first stage, there should not be a
            // link on the second stage
            for job in jobs.iter() {
                for next_job in next_jobs.iter() {
                    if rules_compatible(&job.rules, &next_job.rules) {
                        // link stage to next stage
                        let link = format!("    {} --> {}", stage_name, next_stage_name);
                        mermaid.push(link);
                        // break as we know this stage is compatible
                        break 'stages;
                    }
                }
            }
        }
    }

    Ok(mermaid)
}

fn rules_compatible(
    rules1: &[HashMap<String, CicdEntity>],
    rules2: &[HashMap<String, CicdEntity>],
) -> bool {
    if rules1.is_empty() || rules2.is_empty() {
        return true;
    }
    for rule1 in rules1.iter() {
        for rule2 in rules2.iter() {
            if rule1 == rule2 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cicd_entity_variants() {
        // Test Vec variant
        let vec_entity = CicdEntity::Vec(vec![CicdEntity::Null]);
        assert!(matches!(vec_entity, CicdEntity::Vec(_)));

        // Test Hash variant
        let mut hash_map = HashMap::new();
        hash_map.insert(String::from("key"), CicdEntity::Null);
        let hash_entity = CicdEntity::Hash(hash_map);
        assert!(matches!(hash_entity, CicdEntity::Hash(_)));

        // Test String variant
        let string_entity = CicdEntity::String(String::from("value"));
        assert!(matches!(string_entity, CicdEntity::String(_)));

        // Test Integer variant
        let integer_entity = CicdEntity::Integer(42);
        assert!(matches!(integer_entity, CicdEntity::Integer(_)));

        // Test Null variant
        let null_entity = CicdEntity::Null;
        assert!(matches!(null_entity, CicdEntity::Null));
    }

    #[test]
    fn test_as_vec() {
        let vec_entity = CicdEntity::Vec(vec![CicdEntity::Null]);
        assert!(vec_entity.as_vec().is_some());

        let string_entity = CicdEntity::String(String::from("value"));
        assert!(string_entity.as_vec().is_none());
    }

    #[test]
    fn test_as_hash() {
        let mut hash_map = HashMap::new();
        hash_map.insert(String::from("key"), CicdEntity::Null);
        let hash_entity = CicdEntity::Hash(hash_map);
        assert!(hash_entity.as_hash().is_some());

        let string_entity = CicdEntity::String(String::from("value"));
        assert!(string_entity.as_hash().is_none());
    }

    #[test]
    fn test_as_str() {
        let string_entity = CicdEntity::String(String::from("value"));
        assert_eq!(string_entity.as_str(), Some("value"));

        let integer_entity = CicdEntity::Integer(42);
        assert!(integer_entity.as_str().is_none());
    }

    #[test]
    fn test_index_str() {
        let mut hash_map = HashMap::new();
        hash_map.insert(
            String::from("key"),
            CicdEntity::String(String::from("value")),
        );
        let hash_entity = CicdEntity::Hash(hash_map);

        assert_eq!(
            hash_entity["key"],
            CicdEntity::String(String::from("value"))
        );
        assert_eq!(hash_entity["missing"], CicdEntity::Null);
    }

    #[test]
    fn test_index_usize() {
        let vec_entity = CicdEntity::Vec(vec![CicdEntity::String(String::from("value"))]);

        assert_eq!(vec_entity[0], CicdEntity::String(String::from("value")));
        assert_eq!(vec_entity[1], CicdEntity::Null);
    }

    use std::collections::HashSet;

    #[derive(Clone)]
    struct MockCicdEntity {
        stages: Vec<String>,
        jobs: HashMap<String, CicdEntity>,
    }

    impl MockCicdEntity {
        fn new(stages: Vec<String>, jobs: HashMap<String, CicdEntity>) -> Self {
            Self { stages, jobs }
        }
    }

    impl ToCicdEntity for MockCicdEntity {
        fn get(&self, entity_name: &Option<EntityName>) -> CicdEntity {
            match entity_name {
                Some(EntityName::Stage) => CicdEntity::Vec(
                    self.stages
                        .iter()
                        .map(|s| CicdEntity::String(s.clone()))
                        .collect(),
                ),
                Some(EntityName::Job) => CicdEntity::Hash(self.jobs.clone()),
                None => CicdEntity::Null,
            }
        }
    }

    // Helper function to create a MockCicdEntity
    fn create_mock_cicd_entity(stages: Vec<&str>, jobs: Vec<(&str, CicdEntity)>) -> MockCicdEntity {
        MockCicdEntity::new(
            stages.into_iter().map(String::from).collect(),
            jobs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        )
    }

    #[test]
    fn test_parse_simple_job() {
        let mock = create_mock_cicd_entity(
            vec!["build"],
            vec![(
                "build_job",
                CicdEntity::Hash(HashMap::from([
                    ("stage".to_string(), CicdEntity::String("build".to_string())),
                    (
                        "script".to_string(),
                        CicdEntity::Vec(vec![CicdEntity::String("echo \"Building\"".to_string())]),
                    ),
                ])),
            )],
        );

        let parser = YamlParser::new(mock);
        let mut stage_map = StageMap::new();
        stage_map.insert("build".to_string(), Stage::new("build"));

        parser.get_jobs(&mut stage_map);

        assert_eq!(stage_map.stages["build"].jobs.len(), 1);
        assert_eq!(stage_map.stages["build"].jobs[0].name, "build_job");
    }

    #[test]
    fn test_job_has_non_existing_stage_then_do_not_include() {
        let mock = create_mock_cicd_entity(
            vec!["build"],
            vec![(
                "build_job",
                CicdEntity::Hash(HashMap::from([
                    (
                        "stage".to_string(),
                        CicdEntity::String("non_existing".to_string()),
                    ),
                    (
                        "script".to_string(),
                        CicdEntity::Vec(vec![CicdEntity::String("echo \"Building\"".to_string())]),
                    ),
                ])),
            )],
        );

        let parser = YamlParser::new(mock);
        let mut stage_map = StageMap::new();
        stage_map.insert("build".to_string(), Stage::new("build"));

        parser.get_jobs(&mut stage_map);

        assert_eq!(stage_map.stages["build"].jobs.len(), 0);
    }

    #[test]
    fn test_is_template_not_job() {
        let mock = create_mock_cicd_entity(
            vec!["build"],
            vec![(
                ".build_job_template",
                CicdEntity::Hash(HashMap::from([
                    ("stage".to_string(), CicdEntity::String("build".to_string())),
                    (
                        "script".to_string(),
                        CicdEntity::Vec(vec![CicdEntity::String("echo \"Building\"".to_string())]),
                    ),
                ])),
            )],
        );

        let parser = YamlParser::new(mock);
        let mut stage_map = StageMap::new();
        stage_map.insert("build".to_string(), Stage::new("build"));

        parser.get_jobs(&mut stage_map);

        assert_eq!(stage_map.stages["build"].jobs.len(), 0);
    }

    #[test]
    fn test_parse_job_with_rules() {
        let mock = create_mock_cicd_entity(
            vec!["test"],
            vec![(
                "test_job",
                CicdEntity::Hash(HashMap::from([
                    ("stage".to_string(), CicdEntity::String("test".to_string())),
                    (
                        "script".to_string(),
                        CicdEntity::Vec(vec![CicdEntity::String("echo \"Testing\"".to_string())]),
                    ),
                    (
                        "rules".to_string(),
                        CicdEntity::Vec(vec![CicdEntity::Hash(HashMap::from([(
                            "if".to_string(),
                            CicdEntity::String("$CI_COMMIT_BRANCH == \"main\"".to_string()),
                        )]))]),
                    ),
                ])),
            )],
        );

        let parser = YamlParser::new(mock);

        let mut stage_map = StageMap::new();
        stage_map.insert("test".to_string(), Stage::new("test"));

        parser.get_jobs(&mut stage_map);

        assert_eq!(stage_map.stages["test"].jobs.len(), 1);
        assert_eq!(stage_map.stages["test"].jobs[0].name, "test_job");
        assert_eq!(stage_map.stages["test"].jobs[0].rules.len(), 1);
        assert!(stage_map.stages["test"].jobs[0].rules[0].contains_key("if"));
    }

    #[test]
    fn test_parse_job_with_only_no_refs() {
        let mock = create_mock_cicd_entity(
            vec!["deploy"],
            vec![(
                "deploy_job",
                CicdEntity::Hash(HashMap::from([
                    (
                        "stage".to_string(),
                        CicdEntity::String("deploy".to_string()),
                    ),
                    (
                        "script".to_string(),
                        CicdEntity::Vec(vec![CicdEntity::String("echo \"Deploying\"".to_string())]),
                    ),
                    (
                        "only".to_string(),
                        CicdEntity::Vec(vec![
                            CicdEntity::String("main".to_string()),
                            CicdEntity::String("develop".to_string()),
                        ]),
                    ),
                ])),
            )],
        );

        let parser = YamlParser::new(mock);
        let mut stage_map = StageMap::new();
        stage_map.insert("deploy".to_string(), Stage::new("deploy"));

        parser.get_jobs(&mut stage_map);

        assert_eq!(stage_map.stages["deploy"].jobs.len(), 1);
        assert_eq!(stage_map.stages["deploy"].jobs[0].name, "deploy_job");
        assert_eq!(stage_map.stages["deploy"].jobs[0].rules.len(), 2);

        let rules = &stage_map.stages["deploy"].jobs[0].rules;
        let main_rule = rules.iter().find(|r| r["only"].as_str() == Some("main"));
        let develop_rule = rules.iter().find(|r| r["only"].as_str() == Some("develop"));

        assert!(main_rule.is_some(), "Rule for 'main' branch not found");
        assert!(
            develop_rule.is_some(),
            "Rule for release branches not found"
        );
    }

    #[test]
    fn test_parse_job_with_only_with_refs() {
        let mock = create_mock_cicd_entity(
            vec!["deploy"],
            vec![(
                "deploy_job",
                CicdEntity::Hash(HashMap::from([
                    (
                        "stage".to_string(),
                        CicdEntity::String("deploy".to_string()),
                    ),
                    (
                        "script".to_string(),
                        CicdEntity::Vec(vec![CicdEntity::String("echo \"Deploying\"".to_string())]),
                    ),
                    (
                        "only".to_string(),
                        CicdEntity::Hash(HashMap::from([(
                            "refs".to_string(),
                            CicdEntity::Vec(vec![
                                CicdEntity::String("main".to_string()),
                                CicdEntity::String("/^release-.*$/".to_string()),
                            ]),
                        )])),
                    ),
                ])),
            )],
        );

        let parser = YamlParser::new(mock);
        let mut stage_map = StageMap::new();
        stage_map.insert("deploy".to_string(), Stage::new("deploy"));

        parser.get_jobs(&mut stage_map);

        assert_eq!(stage_map.stages["deploy"].jobs.len(), 1);
        assert_eq!(stage_map.stages["deploy"].jobs[0].name, "deploy_job");
        assert_eq!(stage_map.stages["deploy"].jobs[0].rules.len(), 2);

        let rules = &stage_map.stages["deploy"].jobs[0].rules;
        let main_rule = rules.iter().find(|r| r["only"].as_str() == Some("main"));
        let release_rule = rules
            .iter()
            .find(|r| r["only"].as_str() == Some("/^release-.*$/"));

        assert!(main_rule.is_some(), "Rule for 'main' branch not found");
        assert!(
            release_rule.is_some(),
            "Rule for release branches not found"
        );
    }

    #[test]
    fn test_parse_parallel_jobs() {
        let mock = create_mock_cicd_entity(
            vec!["test"],
            vec![(
                "parallel_job",
                CicdEntity::Hash(HashMap::from([
                    ("stage".to_string(), CicdEntity::String("test".to_string())),
                    (
                        "script".to_string(),
                        CicdEntity::Vec(vec![CicdEntity::String(
                            "echo \"Testing with $PYTHON_VERSION and $DATABASE\"".to_string(),
                        )]),
                    ),
                    (
                        "parallel".to_string(),
                        CicdEntity::Hash(HashMap::from([(
                            "matrix".to_string(),
                            CicdEntity::Vec(vec![CicdEntity::Hash(HashMap::from([
                                (
                                    "PYTHON_VERSION".to_string(),
                                    CicdEntity::Vec(vec![
                                        CicdEntity::String("3.7".to_string()),
                                        CicdEntity::String("3.8".to_string()),
                                    ]),
                                ),
                                (
                                    "DATABASE".to_string(),
                                    CicdEntity::Vec(vec![
                                        CicdEntity::String("mysql".to_string()),
                                        CicdEntity::String("postgres".to_string()),
                                    ]),
                                ),
                            ]))]),
                        )])),
                    ),
                ])),
            )],
        );

        let parser = YamlParser::new(mock);
        let mut stage_map = StageMap::new();
        stage_map.insert("test".to_string(), Stage::new("test"));

        parser.get_jobs(&mut stage_map);

        assert_eq!(stage_map.stages["test"].jobs.len(), 4);

        // Create a set of expected job names
        let expected_job_names: HashSet<String> = [
            "parallel_job-3.7-mysql",
            "parallel_job-3.7-postgres",
            "parallel_job-3.8-mysql",
            "parallel_job-3.8-postgres",
            "parallel_job-mysql-3.7",
            "parallel_job-mysql-3.8",
            "parallel_job-postgres-3.7",
            "parallel_job-postgres-3.8",
        ]
        .iter()
        .map(|&s| s.to_string())
        .collect();

        // Check that each job name in the stage matches one of the expected names
        for job in &stage_map.stages["test"].jobs {
            assert!(
                expected_job_names.contains(&job.name),
                "Unexpected job name: {}",
                job.name
            );
        }

        // Check that we have all combinations of Python versions and databases
        let job_names: HashSet<&String> = stage_map.stages["test"]
            .jobs
            .iter()
            .map(|job| &job.name)
            .collect();
        assert!(job_names
            .iter()
            .any(|name| name.contains("3.7") && name.contains("mysql")));
        assert!(job_names
            .iter()
            .any(|name| name.contains("3.7") && name.contains("postgres")));
        assert!(job_names
            .iter()
            .any(|name| name.contains("3.8") && name.contains("mysql")));
        assert!(job_names
            .iter()
            .any(|name| name.contains("3.8") && name.contains("postgres")));
    }

    #[test]
    fn test_parse_parallel_job_one_element_array() {
        let mock = create_mock_cicd_entity(
            vec!["test"],
            vec![(
                "parallel_job",
                CicdEntity::Hash(HashMap::from([
                    ("stage".to_string(), CicdEntity::String("test".to_string())),
                    (
                        "script".to_string(),
                        CicdEntity::Vec(vec![CicdEntity::String(
                            "echo \"Testing with $RUST_VERSION\"".to_string(),
                        )]),
                    ),
                    (
                        "parallel".to_string(),
                        CicdEntity::Hash(HashMap::from([(
                            "matrix".to_string(),
                            CicdEntity::Vec(vec![CicdEntity::Hash(HashMap::from([(
                                "RUST_VERSION".to_string(),
                                CicdEntity::Vec(vec![
                                    CicdEntity::String("1.50".to_string()),
                                    CicdEntity::String("1.60".to_string()),
                                ]),
                            )]))]),
                        )])),
                    ),
                ])),
            )],
        );

        let parser = YamlParser::new(mock);
        let mut stage_map = StageMap::new();
        stage_map.insert("test".to_string(), Stage::new("test"));

        parser.get_jobs(&mut stage_map);

        assert_eq!(stage_map.stages["test"].jobs.len(), 2);

        // Create a set of expected job names
        let expected_job_names: HashSet<String> = ["parallel_job-1.50", "parallel_job-1.60"]
            .iter()
            .map(|&s| s.to_string())
            .collect();

        // Check that each job name in the stage matches one of the expected names
        for job in &stage_map.stages["test"].jobs {
            assert!(
                expected_job_names.contains(&job.name),
                "Unexpected job name: {}",
                job.name
            );
        }

        // Check that we have all combinations of Python versions and databases
        let job_names: HashSet<&String> = stage_map.stages["test"]
            .jobs
            .iter()
            .map(|job| &job.name)
            .collect();
        assert!(job_names.iter().any(|name| name.contains("1.50")));
        assert!(job_names.iter().any(|name| name.contains("1.60")));
    }

    #[test]
    fn test_get_stages() {
        let mock = create_mock_cicd_entity(
            vec!["build", "test", "deploy"],
            vec![], // We don't need job definitions for this test
        );

        let parser = YamlParser::new(mock);
        let stage_map = parser.get_stages().unwrap();

        assert_eq!(stage_map.stage_names.len(), 3);
        assert_eq!(stage_map.stage_names[0], "build");
        assert_eq!(stage_map.stage_names[1], "test");
        assert_eq!(stage_map.stage_names[2], "deploy");
    }

    // Mermaid testing

    struct MockParser {
        stages: Vec<String>,
        jobs: HashMap<String, Vec<MockJob>>,
    }

    struct MockJob {
        name: String,
        rules: Vec<HashMap<String, CicdEntity>>,
    }

    impl CicdParser for MockParser {
        fn get_stages(&self) -> Result<StageMap> {
            let mut map = StageMap::new();
            for stage in &self.stages {
                map.insert(stage.clone(), Stage::new(stage));
            }
            Ok(map)
        }

        fn get_jobs(&self, stages: &mut StageMap) {
            for (stage_name, mock_jobs) in &self.jobs {
                if let Some(stage) = stages.get_mut(stage_name) {
                    stage.jobs = mock_jobs
                        .iter()
                        .map(|mock_job| Job::new(&mock_job.name, mock_job.rules.clone()))
                        .collect();
                }
            }
        }
    }

    fn create_mock_parser(
        stages: Vec<&str>,
        jobs: Vec<(&str, Vec<(&str, Vec<HashMap<String, CicdEntity>>)>)>,
    ) -> MockParser {
        let stages = stages.into_iter().map(String::from).collect();
        let jobs = jobs
            .into_iter()
            .map(|(stage, job_specs)| {
                (
                    stage.to_string(),
                    job_specs
                        .into_iter()
                        .map(|(name, rules)| MockJob {
                            name: name.to_string(),
                            rules,
                        })
                        .collect(),
                )
            })
            .collect();
        MockParser { stages, jobs }
    }

    #[test]
    fn test_simple_pipeline() -> Result<()> {
        let parser = create_mock_parser(
            vec!["build", "test", "deploy"],
            vec![
                ("build", vec![("compile", vec![])]),
                (
                    "test",
                    vec![("unit-test", vec![]), ("integration-test", vec![])],
                ),
                ("deploy", vec![("production", vec![])]),
            ],
        );

        let mermaid = generate_mermaid_stages_diagram(parser)?;
        let diagram = mermaid.to_string();

        assert!(diagram.contains("stateDiagram-v2"));
        assert!(diagram.contains("direction LR"));
        assert!(diagram.contains("state build{"));
        assert!(diagram.contains("state test{"));
        assert!(diagram.contains("state deploy{"));
        assert!(diagram.contains("build --> test"));
        assert!(diagram.contains("test --> deploy"));
        assert!(diagram.contains("state \"compile\" as anchorT0"));
        assert!(diagram.contains("state \"unit-test\" as anchorT1"));
        assert!(diagram.contains("state \"integration-test\" as anchorT1"));
        assert!(diagram.contains("state \"production\" as anchorT2"));

        Ok(())
    }

    #[test]
    fn test_pipeline_with_empty_stage() -> Result<()> {
        let parser = create_mock_parser(
            vec!["build", "test", "deploy"],
            vec![
                ("build", vec![("compile", vec![])]),
                ("test", vec![]),
                ("deploy", vec![("production", vec![])]),
            ],
        );

        let mermaid = generate_mermaid_stages_diagram(parser)?;
        let diagram = mermaid.to_string();

        assert!(diagram.contains("build --> deploy"));
        assert!(!diagram.contains("test -->"));

        Ok(())
    }

    #[test]
    fn test_pipeline_with_rules() -> Result<()> {
        let parser = create_mock_parser(
            vec!["build", "test", "deploy"],
            vec![
                (
                    "build",
                    vec![(
                        "compile",
                        vec![HashMap::from([(
                            "only".to_string(),
                            CicdEntity::String("main".to_string()),
                        )])],
                    )],
                ),
                (
                    "test",
                    vec![(
                        "unit-test",
                        vec![HashMap::from([(
                            "only".to_string(),
                            CicdEntity::String("main".to_string()),
                        )])],
                    )],
                ),
                (
                    "deploy",
                    vec![(
                        "production",
                        vec![HashMap::from([(
                            "only".to_string(),
                            CicdEntity::String("tags".to_string()),
                        )])],
                    )],
                ),
            ],
        );

        let mermaid = generate_mermaid_stages_diagram(parser)?;
        let diagram = mermaid.to_string();

        assert!(diagram.contains("build --> test"));
        assert!(!diagram.contains("test --> deploy"));

        Ok(())
    }

    #[test]
    fn test_pipeline_with_pre_and_post_stages() -> Result<()> {
        let parser = create_mock_parser(
            vec![".pre", "build", "test", ".post"],
            vec![
                (".pre", vec![("setup", vec![])]),
                ("build", vec![("compile", vec![])]),
                ("test", vec![("unit-test", vec![])]),
                (".post", vec![("cleanup", vec![])]),
            ],
        );

        let mermaid = generate_mermaid_stages_diagram(parser)?;
        let diagram = mermaid.to_string();

        assert!(diagram.contains("state .pre{"));
        assert!(diagram.contains("state .post{"));
        assert!(diagram.contains(".pre --> build"));
        assert!(diagram.contains("test --> .post"));
        assert!(diagram.contains("state build{"));
        assert!(diagram.contains("state test{"));
        assert!(diagram.contains("state \"setup\" as anchorT0"));
        assert!(diagram.contains("state \"cleanup\" as anchorT3"));

        Ok(())
    }

    #[test]
    fn test_pipeline_with_empty_pre_and_post_stages() -> Result<()> {
        let parser = create_mock_parser(
            vec![".pre", "build", "test", ".post"],
            vec![
                (".pre", vec![]),
                ("build", vec![("compile", vec![])]),
                ("test", vec![("unit-test", vec![])]),
                (".post", vec![]),
            ],
        );

        let mermaid = generate_mermaid_stages_diagram(parser)?;
        let diagram = mermaid.to_string();

        assert!(!diagram.contains("state .pre{"));
        assert!(!diagram.contains("state .post{"));
        assert!(diagram.contains("build --> test"));
        assert!(diagram.contains("state build{"));
        assert!(diagram.contains("state test{"));

        Ok(())
    }

    #[test]
    fn test_pipeline_with_long_names() -> Result<()> {
        let parser = create_mock_parser(
            vec!["build-and-compile", "run-all-tests", "deploy-to-production"],
            vec![
                ("build-and-compile", vec![("compile-source-code", vec![])]),
                (
                    "run-all-tests",
                    vec![
                        ("run-unit-tests", vec![]),
                        ("run-integration-tests", vec![]),
                    ],
                ),
                (
                    "deploy-to-production",
                    vec![("deploy-to-prod-servers", vec![])],
                ),
            ],
        );

        let mermaid = generate_mermaid_stages_diagram(parser)?;
        let diagram = mermaid.to_string();

        assert!(diagram.contains("state build_and_compile{"));
        assert!(diagram.contains("state run_all_tests{"));
        assert!(diagram.contains("state deploy_to_production{"));
        assert!(diagram.contains("build_and_compile --> run_all_tests"));
        assert!(diagram.contains("run_all_tests --> deploy_to_production"));

        Ok(())
    }
}
