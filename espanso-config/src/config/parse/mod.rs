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

use anyhow::Result;
use std::{convert::TryInto, path::Path};
use thiserror::Error;

mod yaml;

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ParsedConfig {
  pub label: Option<String>,

  pub backend: Option<String>,

  // Includes
  pub includes: Option<Vec<String>>,
  pub excludes: Option<Vec<String>>,
  pub extra_includes: Option<Vec<String>>,
  pub extra_excludes: Option<Vec<String>>,
  pub use_standard_includes: Option<bool>,

  // Filters
  pub filter_title: Option<String>,
  pub filter_class: Option<String>,
  pub filter_exec: Option<String>,
  pub filter_os: Option<String>,
}

impl ParsedConfig {
  pub fn load(path: &Path) -> Result<Self> {
    let content = std::fs::read_to_string(path)?;
    match yaml::YAMLConfig::parse_from_str(&content) {
      Ok(config) => Ok(config.try_into()?),
      Err(err) => Err(ParsedConfigError::LoadFailed(err).into()),
    }
  }
}

#[derive(Error, Debug)]
pub enum ParsedConfigError {
  #[error("can't load config `{0}`")]
  LoadFailed(#[from] anyhow::Error),
}