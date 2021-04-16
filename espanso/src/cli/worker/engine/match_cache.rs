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

use std::{collections::HashMap, iter::FromIterator};

use espanso_config::{
  config::ConfigStore,
  matches::{store::MatchStore, Match},
};

use super::{multiplex::MatchProvider, render::MatchIterator};

pub struct MatchCache<'a> {
  cache: HashMap<i32, &'a Match>,
}

impl<'a> MatchCache<'a> {
  pub fn load(config_store: &'a dyn ConfigStore, match_store: &'a dyn MatchStore) -> Self {
    let mut cache = HashMap::new();

    let paths = config_store.get_all_match_paths();
    let global_set = match_store.query(&Vec::from_iter(paths.into_iter()));

    for m in global_set.matches {
      cache.insert(m.id, m);
    }

    Self { cache }
  }
}

impl<'a> MatchProvider<'a> for MatchCache<'a> {
  fn get(&self, match_id: i32) -> Option<&'a Match> {
    self.cache.get(&match_id).map(|m| *m)
  }
}

impl<'a> MatchIterator<'a> for MatchCache<'a> {
  fn matches(&self) -> Vec<&'a Match> {
    self.cache.iter().map(|(_, m)| *m).collect()
  }
}