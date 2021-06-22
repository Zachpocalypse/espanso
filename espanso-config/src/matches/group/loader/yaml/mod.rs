/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * espanso is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with espanso.  If not, see <https://www.gnu.org/licenses/>.
 */

use crate::{
  counter::next_id,
  matches::{
    group::{path::resolve_imports, MatchGroup},
    ImageEffect, Match, Params, RegexCause, TextFormat, TextInjectMode, UpperCasingStyle, Value,
    Variable,
  },
};
use anyhow::Result;
use log::{error, warn};
use parse::YAMLMatchGroup;
use regex::{Captures, Regex};
use std::convert::{TryFrom, TryInto};

use self::{
  parse::{YAMLMatch, YAMLVariable},
  util::convert_params,
};
use crate::matches::{MatchCause, MatchEffect, TextEffect, TriggerCause};

use super::Importer;

pub(crate) mod parse;
mod util;

lazy_static! {
  static ref VAR_REGEX: Regex = Regex::new("\\{\\{\\s*(\\w+)(\\.\\w+)?\\s*\\}\\}").unwrap();
}

pub(crate) struct YAMLImporter {}

impl YAMLImporter {
  pub fn new() -> Self {
    Self {}
  }
}

impl Importer for YAMLImporter {
  fn is_supported(&self, extension: &str) -> bool {
    extension == "yaml" || extension == "yml"
  }

  fn load_group(
    &self,
    path: &std::path::Path,
  ) -> anyhow::Result<crate::matches::group::MatchGroup> {
    let yaml_group = YAMLMatchGroup::parse_from_file(path)?;

    let global_vars: Result<Vec<Variable>> = yaml_group
      .global_vars
      .as_ref()
      .cloned()
      .unwrap_or_default()
      .iter()
      .map(|var| var.clone().try_into())
      .collect();

    let matches: Result<Vec<Match>> = yaml_group
      .matches
      .as_ref()
      .cloned()
      .unwrap_or_default()
      .iter()
      .map(|m| m.clone().try_into())
      .collect();

    // Resolve imports
    let resolved_imports = resolve_imports(path, &yaml_group.imports.unwrap_or_default())?;

    Ok(MatchGroup {
      imports: resolved_imports,
      global_vars: global_vars?,
      matches: matches?,
    })
  }
}

impl TryFrom<YAMLMatch> for Match {
  type Error = anyhow::Error;

  fn try_from(yaml_match: YAMLMatch) -> Result<Self, Self::Error> {
    if yaml_match.uppercase_style.is_some() && yaml_match.propagate_case.is_none() {
      warn!("specifying the 'uppercase_style' option without 'propagate_case' has no effect");
    }

    let triggers = if let Some(trigger) = yaml_match.trigger {
      Some(vec![trigger])
    } else if let Some(triggers) = yaml_match.triggers {
      Some(triggers)
    } else {
      None
    };

    let uppercase_style = match yaml_match
      .uppercase_style
      .map(|s| s.to_lowercase())
      .as_deref()
    {
      Some("uppercase") => UpperCasingStyle::Uppercase,
      Some("capitalize") => UpperCasingStyle::Capitalize,
      Some("capitalize_words") => UpperCasingStyle::CapitalizeWords,
      Some(style) => {
        error!(
          "unrecognized uppercase_style: {:?}, falling back to the default",
          style
        );
        TriggerCause::default().uppercase_style
      }
      _ => TriggerCause::default().uppercase_style,
    };

    let cause = if let Some(triggers) = triggers {
      MatchCause::Trigger(TriggerCause {
        triggers,
        left_word: yaml_match
          .left_word
          .or(yaml_match.word)
          .unwrap_or(TriggerCause::default().left_word),
        right_word: yaml_match
          .right_word
          .or(yaml_match.word)
          .unwrap_or(TriggerCause::default().right_word),
        propagate_case: yaml_match
          .propagate_case
          .unwrap_or(TriggerCause::default().propagate_case),
        uppercase_style,
      })
    } else if let Some(regex) = yaml_match.regex {
      // TODO: add test case
      MatchCause::Regex(RegexCause { regex })
    } else {
      MatchCause::None
    };

    // TODO: test force_mode/force_clipboard
    let force_mode = if let Some(true) = yaml_match.force_clipboard {
      Some(TextInjectMode::Clipboard)
    } else if let Some(mode) = yaml_match.force_mode {
      match mode.to_lowercase().as_str() {
        "clipboard" => Some(TextInjectMode::Clipboard),
        "keys" => Some(TextInjectMode::Keys),
        _ => None,
      }
    } else {
      None
    };

    let effect =
      if yaml_match.replace.is_some() || yaml_match.markdown.is_some() || yaml_match.html.is_some()
      {
        // TODO: test markdown and html cases
        let (replace, format) = if let Some(plain) = yaml_match.replace {
          (plain, TextFormat::Plain)
        } else if let Some(markdown) = yaml_match.markdown {
          (markdown, TextFormat::Markdown)
        } else if let Some(html) = yaml_match.html {
          (html, TextFormat::Html)
        } else {
          unreachable!();
        };

        let vars: Result<Vec<Variable>> = yaml_match
          .vars
          .unwrap_or_default()
          .into_iter()
          .map(|var| var.try_into())
          .collect();

        MatchEffect::Text(TextEffect {
          replace,
          vars: vars?,
          format,
          force_mode,
        })
      } else if let Some(form_layout) = yaml_match.form {
        // TODO: test form case
        // Replace all the form fields with actual variables
        let resolved_layout = VAR_REGEX
          .replace_all(&form_layout, |caps: &Captures| {
            let var_name = caps.get(1).unwrap().as_str();
            format!("{{{{form1.{}}}}}", var_name)
          })
          .to_string();

        // Convert escaped brakets in forms
        let resolved_layout = resolved_layout.replace("\\{", "{ ").replace("\\}", " }");

        // Convert the form data to valid variables
        let mut params = Params::new();
        params.insert("layout".to_string(), Value::String(form_layout));

        if let Some(fields) = yaml_match.form_fields {
          params.insert("fields".to_string(), Value::Object(convert_params(fields)?));
        }

        let vars = vec![Variable {
          id: next_id(),
          name: "form1".to_owned(),
          var_type: "form".to_owned(),
          params,
        }];

        MatchEffect::Text(TextEffect {
          replace: resolved_layout,
          vars,
          format: TextFormat::Plain,
          force_mode,
        })
      } else if let Some(image_path) = yaml_match.image_path {
        // TODO: test image case
        MatchEffect::Image(ImageEffect { path: image_path })
      } else {
        MatchEffect::None
      };

    if let MatchEffect::None = effect {
      warn!(
        "match caused by {:?} does not produce any effect. Did you forget the 'replace' field?",
        cause
      );
    }

    Ok(Self {
      cause,
      effect,
      label: None,
      id: next_id(),
    })
  }
}

impl TryFrom<YAMLVariable> for Variable {
  type Error = anyhow::Error;

  fn try_from(yaml_var: YAMLVariable) -> Result<Self, Self::Error> {
    Ok(Self {
      name: yaml_var.name,
      var_type: yaml_var.var_type,
      params: convert_params(yaml_var.params)?,
      id: next_id(),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    matches::{Match, Params, Value},
    util::tests::use_test_directory,
  };
  use std::fs::create_dir_all;

  fn create_match(yaml: &str) -> Result<Match> {
    let yaml_match: YAMLMatch = serde_yaml::from_str(yaml)?;
    let mut m: Match = yaml_match.try_into()?;

    // Reset the IDs to correctly compare them
    m.id = 0;
    if let MatchEffect::Text(e) = &mut m.effect {
      e.vars.iter_mut().for_each(|v| v.id = 0);
    }

    Ok(m)
  }

  #[test]
  fn basic_match_maps_correctly() {
    assert_eq!(
      create_match(
        r#"
        trigger: "Hello"
        replace: "world"
        "#
      )
      .unwrap(),
      Match {
        cause: MatchCause::Trigger(TriggerCause {
          triggers: vec!["Hello".to_string()],
          ..Default::default()
        }),
        effect: MatchEffect::Text(TextEffect {
          replace: "world".to_string(),
          ..Default::default()
        }),
        ..Default::default()
      }
    )
  }

  #[test]
  fn multiple_triggers_maps_correctly() {
    assert_eq!(
      create_match(
        r#"
        triggers: ["Hello", "john"]
        replace: "world"
        "#
      )
      .unwrap(),
      Match {
        cause: MatchCause::Trigger(TriggerCause {
          triggers: vec!["Hello".to_string(), "john".to_string()],
          ..Default::default()
        }),
        effect: MatchEffect::Text(TextEffect {
          replace: "world".to_string(),
          ..Default::default()
        }),
        ..Default::default()
      }
    )
  }

  #[test]
  fn word_maps_correctly() {
    assert_eq!(
      create_match(
        r#"
        trigger: "Hello"
        replace: "world"
        word: true
        "#
      )
      .unwrap(),
      Match {
        cause: MatchCause::Trigger(TriggerCause {
          triggers: vec!["Hello".to_string()],
          left_word: true,
          right_word: true,
          ..Default::default()
        }),
        effect: MatchEffect::Text(TextEffect {
          replace: "world".to_string(),
          ..Default::default()
        }),
        ..Default::default()
      }
    )
  }

  #[test]
  fn left_word_maps_correctly() {
    assert_eq!(
      create_match(
        r#"
        trigger: "Hello"
        replace: "world"
        left_word: true
        "#
      )
      .unwrap(),
      Match {
        cause: MatchCause::Trigger(TriggerCause {
          triggers: vec!["Hello".to_string()],
          left_word: true,
          ..Default::default()
        }),
        effect: MatchEffect::Text(TextEffect {
          replace: "world".to_string(),
          ..Default::default()
        }),
        ..Default::default()
      }
    )
  }

  #[test]
  fn right_word_maps_correctly() {
    assert_eq!(
      create_match(
        r#"
        trigger: "Hello"
        replace: "world"
        right_word: true
        "#
      )
      .unwrap(),
      Match {
        cause: MatchCause::Trigger(TriggerCause {
          triggers: vec!["Hello".to_string()],
          right_word: true,
          ..Default::default()
        }),
        effect: MatchEffect::Text(TextEffect {
          replace: "world".to_string(),
          ..Default::default()
        }),
        ..Default::default()
      }
    )
  }

  #[test]
  fn propagate_case_maps_correctly() {
    assert_eq!(
      create_match(
        r#"
        trigger: "Hello"
        replace: "world"
        propagate_case: true
        "#
      )
      .unwrap(),
      Match {
        cause: MatchCause::Trigger(TriggerCause {
          triggers: vec!["Hello".to_string()],
          propagate_case: true,
          ..Default::default()
        }),
        effect: MatchEffect::Text(TextEffect {
          replace: "world".to_string(),
          ..Default::default()
        }),
        ..Default::default()
      }
    )
  }

  #[test]
  fn uppercase_style_maps_correctly() {
    assert_eq!(
      create_match(
        r#"
        trigger: "Hello"
        replace: "world"
        uppercase_style: "capitalize"
        "#
      )
      .unwrap()
      .cause
      .into_trigger()
      .unwrap()
      .uppercase_style,
      UpperCasingStyle::Capitalize,
    );

    assert_eq!(
      create_match(
        r#"
        trigger: "Hello"
        replace: "world"
        uppercase_style: "capitalize_words"
        "#
      )
      .unwrap()
      .cause
      .into_trigger()
      .unwrap()
      .uppercase_style,
      UpperCasingStyle::CapitalizeWords,
    );

    assert_eq!(
      create_match(
        r#"
        trigger: "Hello"
        replace: "world"
        uppercase_style: "uppercase"
        "#
      )
      .unwrap()
      .cause
      .into_trigger()
      .unwrap()
      .uppercase_style,
      UpperCasingStyle::Uppercase,
    );

    assert_eq!(
      create_match(
        r#"
        trigger: "Hello"
        replace: "world"
        uppercase_style: "invalid"
        "#
      )
      .unwrap()
      .cause
      .into_trigger()
      .unwrap()
      .uppercase_style,
      UpperCasingStyle::Uppercase,
    );
  }

  #[test]
  fn vars_maps_correctly() {
    let mut params = Params::new();
    params.insert("param1".to_string(), Value::Bool(true));
    let vars = vec![Variable {
      name: "var1".to_string(),
      var_type: "test".to_string(),
      params,
      ..Default::default()
    }];
    assert_eq!(
      create_match(
        r#"
        trigger: "Hello"
        replace: "world"
        vars:
          - name: var1
            type: test
            params:
              param1: true
        "#
      )
      .unwrap(),
      Match {
        cause: MatchCause::Trigger(TriggerCause {
          triggers: vec!["Hello".to_string()],
          ..Default::default()
        }),
        effect: MatchEffect::Text(TextEffect {
          replace: "world".to_string(),
          vars,
          ..Default::default()
        }),
        ..Default::default()
      }
    )
  }

  #[test]
  fn vars_no_params_maps_correctly() {
    let vars = vec![Variable {
      name: "var1".to_string(),
      var_type: "test".to_string(),
      params: Params::new(),
      ..Default::default()
    }];
    assert_eq!(
      create_match(
        r#"
        trigger: "Hello"
        replace: "world"
        vars:
          - name: var1
            type: test
        "#
      )
      .unwrap(),
      Match {
        cause: MatchCause::Trigger(TriggerCause {
          triggers: vec!["Hello".to_string()],
          ..Default::default()
        }),
        effect: MatchEffect::Text(TextEffect {
          replace: "world".to_string(),
          vars,
          ..Default::default()
        }),
        ..Default::default()
      }
    )
  }

  #[test]
  fn importer_is_supported() {
    let importer = YAMLImporter::new();
    assert!(importer.is_supported("yaml"));
    assert!(importer.is_supported("yml"));
    assert!(!importer.is_supported("invalid"));
  }

  #[test]
  fn importer_works_correctly() {
    use_test_directory(|_, match_dir, _| {
      let sub_dir = match_dir.join("sub");
      create_dir_all(&sub_dir).unwrap();

      let base_file = match_dir.join("base.yml");
      std::fs::write(
        &base_file,
        r#"
      imports:
        - "sub/sub.yml"
        - "invalid/import.yml" # This should be discarded
      
      global_vars:
        - name: "var1"
          type: "test"
      
      matches:
        - trigger: "hello"
          replace: "world"
      "#,
      )
      .unwrap();

      let sub_file = sub_dir.join("sub.yml");
      std::fs::write(&sub_file, "").unwrap();

      let importer = YAMLImporter::new();
      let mut group = importer.load_group(&base_file).unwrap();
      // Reset the ids to compare them correctly
      group.matches.iter_mut().for_each(|mut m| m.id = 0);
      group.global_vars.iter_mut().for_each(|mut v| v.id = 0);

      let vars = vec![Variable {
        name: "var1".to_string(),
        var_type: "test".to_string(),
        params: Params::new(),
        ..Default::default()
      }];

      assert_eq!(
        group,
        MatchGroup {
          imports: vec![sub_file.to_string_lossy().to_string(),],
          global_vars: vars,
          matches: vec![Match {
            cause: MatchCause::Trigger(TriggerCause {
              triggers: vec!["hello".to_string()],
              ..Default::default()
            }),
            effect: MatchEffect::Text(TextEffect {
              replace: "world".to_string(),
              ..Default::default()
            }),
            ..Default::default()
          }],
        }
      )
    });
  }
}
