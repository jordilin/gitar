use std::collections::HashMap;

use yaml_rust2::Yaml;

use super::mermaid::{CicdEntity, EntityName, ToCicdEntity};

impl ToCicdEntity for Yaml {
    fn get(&self, entity_name: &Option<EntityName>) -> CicdEntity {
        let value = if let Some(entity_name) = entity_name {
            match entity_name {
                EntityName::Stage => &self["stages"],
                EntityName::Job => self,
            }
        } else {
            self
        };
        match value {
            Yaml::Array(ref v) => {
                let mut vec = Vec::new();
                for item in v {
                    vec.push(item.get(&None::<EntityName>));
                }
                CicdEntity::Vec(vec)
            }
            Yaml::Hash(ref h) => {
                let mut hash = HashMap::new();
                for (key, value) in h {
                    hash.insert(
                        key.as_str().unwrap().to_string(),
                        value.get(&None::<EntityName>),
                    );
                }
                CicdEntity::Hash(hash)
            }
            Yaml::String(ref s) => CicdEntity::String(s.to_string()),
            Yaml::Integer(ref i) => CicdEntity::Integer(*i),
            _ => CicdEntity::Null,
        }
    }
}

pub fn load_yaml(yaml: &str) -> Yaml {
    yaml_rust2::YamlLoader::load_from_str(yaml)
        .unwrap()
        .pop()
        .unwrap()
}

#[cfg(test)]
mod tests {
    use crate::cmds::cicd::mermaid::{CicdParser, YamlParser};

    use super::*;

    #[test]
    fn test_read_simple_gitlab_ci_yaml() {
        let yaml = r#"stages:
- build

build:
  stage: build
  script:
  - echo "Building the app...""#;

        let yaml_obj = load_yaml(yaml);
        let parser = YamlParser::new(yaml_obj);
        let stages = parser.get_stages().unwrap();
        assert_eq!(stages.len(), 1);
        assert_eq!(stages[0].name, "build");
    }

    fn create_yaml(yaml_str: &str) -> Yaml {
        yaml_rust2::YamlLoader::load_from_str(yaml_str)
            .unwrap()
            .pop()
            .unwrap()
    }

    #[test]
    fn test_yaml_array_to_cicd_entity() {
        let yaml = create_yaml("[1, 'test', [nested]]");
        let result = yaml.get(&None);
        assert!(matches!(result, CicdEntity::Vec(_)));
        if let CicdEntity::Vec(vec) = result {
            assert_eq!(vec.len(), 3);
            assert!(matches!(vec[0], CicdEntity::Integer(1)));
            assert!(matches!(vec[1], CicdEntity::String(ref s) if s == "test"));
            assert!(matches!(vec[2], CicdEntity::Vec(_)));
        }
    }

    #[test]
    fn test_yaml_hash_to_cicd_entity() {
        let yaml = create_yaml("key1: value1\nkey2: 2");
        let result = yaml.get(&None);
        assert!(matches!(result, CicdEntity::Hash(_)));
        if let CicdEntity::Hash(hash) = result {
            assert_eq!(hash.len(), 2);
            assert!(matches!(hash.get("key1"), Some(CicdEntity::String(ref s)) if s == "value1"));
            assert!(matches!(hash.get("key2"), Some(CicdEntity::Integer(2))));
        }
    }

    #[test]
    fn test_yaml_string_to_cicd_entity() {
        let yaml = create_yaml("'test string'");
        let result = yaml.get(&None);
        assert!(matches!(result, CicdEntity::String(ref s) if s == "test string"));
    }

    #[test]
    fn test_yaml_integer_to_cicd_entity() {
        let yaml = create_yaml("42");
        let result = yaml.get(&None);
        assert!(matches!(result, CicdEntity::Integer(42)));
    }

    #[test]
    fn test_yaml_null_to_cicd_entity() {
        let yaml = create_yaml("~");
        let result = yaml.get(&None);
        assert!(matches!(result, CicdEntity::Null));
    }

    #[test]
    fn test_yaml_boolean_to_cicd_entity() {
        let yaml = create_yaml("true");
        let result = yaml.get(&None);
        assert!(matches!(result, CicdEntity::Null));
    }

    #[test]
    fn test_yaml_float_to_cicd_entity() {
        let yaml = create_yaml("3.14");
        let result = yaml.get(&None);
        assert!(matches!(result, CicdEntity::Null));
    }

    #[test]
    fn test_yaml_to_cicd_entity_with_stage_entity_name() {
        let yaml = create_yaml("stages: [build, test, deploy]\nother: value");
        let result = yaml.get(&Some(EntityName::Stage));
        assert!(matches!(result, CicdEntity::Vec(_)));
        if let CicdEntity::Vec(vec) = result {
            assert_eq!(vec.len(), 3);
            assert!(matches!(vec[0], CicdEntity::String(ref s) if s == "build"));
            assert!(matches!(vec[1], CicdEntity::String(ref s) if s == "test"));
            assert!(matches!(vec[2], CicdEntity::String(ref s) if s == "deploy"));
        }
    }

    #[test]
    fn test_yaml_to_cicd_entity_with_job_entity_name() {
        let yaml =
            create_yaml("stages: [build, test]\njob1:\n  stage: build\n  script: echo 'test'");
        let result = yaml.get(&Some(EntityName::Job));
        assert!(matches!(result, CicdEntity::Hash(_)));
        if let CicdEntity::Hash(hash) = result {
            assert_eq!(hash.len(), 2);
            assert!(hash.contains_key("stages"));
            assert!(hash.contains_key("job1"));
        }
    }

    #[test]
    fn test_yaml_to_cicd_entity_nested_structures() {
        let yaml = create_yaml(
            "
            key1:
              nested1: value1
              nested2: [1, 2, 3]
            key2:
              - item1
              - subitem:
                  subsubitem: value2
        ",
        );
        let result = yaml.get(&None);
        assert!(matches!(result, CicdEntity::Hash(_)));
        if let CicdEntity::Hash(hash) = result {
            assert_eq!(hash.len(), 2);
            assert!(matches!(hash.get("key1"), Some(CicdEntity::Hash(_))));
            assert!(matches!(hash.get("key2"), Some(CicdEntity::Vec(_))));
        }
    }

    #[test]
    fn test_yaml_to_cicd_entity_empty_structures() {
        let yaml = create_yaml("empty_array: []\nempty_hash: {}");
        let result = yaml.get(&None);
        assert!(matches!(result, CicdEntity::Hash(_)));
        if let CicdEntity::Hash(hash) = result {
            assert_eq!(hash.len(), 2);
            assert!(
                matches!(hash.get("empty_array"), Some(CicdEntity::Vec(vec)) if vec.is_empty())
            );
            assert!(matches!(hash.get("empty_hash"), Some(CicdEntity::Hash(h)) if h.is_empty()));
        }
    }
}
