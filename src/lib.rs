use napi::Status::GenericFailure;
use napi::{Env, Error, Result};
use napi_derive::napi;
use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_module_lexer::ModuleLexer;
use oxc_parser::Parser;
use oxc_resolver::{AliasValue, ResolveOptions, Resolver, TsconfigOptions, TsconfigReferences};
use oxc_span::SourceType;
use pathdiff::diff_paths;
use regex::Regex;
use std::collections::HashSet;
use std::path::PathBuf;

pub fn is_bare_module_specifier(specifier: &str) -> bool {
  let specifier = specifier.replace('\'', "");
  if let Some(first_char) = specifier.chars().next() {
    let re = Regex::new(r"[@a-zA-Z]").unwrap();
    return re.is_match(&first_char.to_string());
  }
  false
}

#[napi]
pub fn resolve_rs(
  _env: Env,
  importer: String,
  importee: String,
  condition_names: Vec<String>,
  main_fields: Vec<String>,
  extensions: Vec<String>,
  tsconfig_config_file: Option<String>,
  tsconfig_references: Option<Vec<String>>,
) -> Result<String> {
  let tsconfig = match tsconfig_config_file {
    None => None,
    _ => Some(create_tsconfig_option(
      tsconfig_config_file.unwrap(),
      tsconfig_references,
    )),
  };
  let options: ResolveOptions = ResolveOptions {
    tsconfig,
    condition_names,
    main_fields,
    extensions,
    ..ResolveOptions::default()
  };
  let resolver = Resolver::new(options);

  let importer_path = PathBuf::from(&importer);
  let importer_parent = importer_path.parent().unwrap().to_str().unwrap();

  let resolved_url = match resolver.resolve(importer_parent, &importee) {
    Ok(url) => url,
    Err(_) => {
      return Err(Error::new(
        GenericFailure,
        format!(
          "Failed to resolve importer: \"{}\", importee: \"{}\"",
          &importer, &importee
        ),
      ));
    }
  };
  Ok(resolved_url.path().to_str().unwrap().to_string())
}

#[napi]
pub fn is_barrel_file_rs(
  _env: Env,
  source: String,
  amount_of_exports_to_consider_module_as_barrel: u32,
) -> Result<bool> {
  let allocator = Allocator::default();
  let ret = Parser::new(&allocator, &source, SourceType::default()).parse();
  let ModuleLexer { exports, .. } = ModuleLexer::new().build(&ret.program);

  let mut declarations = 0;
  for declaration in ret.program.body {
    match declaration {
      Statement::VariableDeclaration(variable) => {
        declarations += variable.declarations.len();
      }
      Statement::FunctionDeclaration(_) => {
        declarations += 1;
      }
      Statement::ClassDeclaration(_) => {
        declarations += 1;
      }
      _ => {}
    }
  }

  if declarations < exports.len()
    && exports.len() > amount_of_exports_to_consider_module_as_barrel as usize
  {
    return Ok(true);
  }
  Ok(false)
}

fn create_tsconfig_option(
  config_file: String,
  tsconfig_references: Option<Vec<String>>,
) -> TsconfigOptions {
  TsconfigOptions {
    config_file: PathBuf::from(config_file),
    references: match tsconfig_references {
      None => TsconfigReferences::Auto,
      Some(refs) => TsconfigReferences::Paths(refs.into_iter().map(PathBuf::from).collect()),
    },
  }
}

fn create_alias_option(aliases: Vec<(String, Vec<String>)>) -> Vec<(String, Vec<AliasValue>)> {
  aliases
    .into_iter()
    .map(|(key, val)| {
      let mapped_alias = if val.is_empty() {
        vec![AliasValue::Ignore]
      } else {
        val.into_iter().map(AliasValue::Path).collect()
      };

      (key, mapped_alias)
    })
    .collect()
}

#[napi]
pub fn count_module_graph_size_rs(
  _env: Env,
  entry_points: Vec<String>,
  base_path: String,
  condition_names: Vec<String>,
  main_fields: Vec<String>,
  extensions: Vec<String>,
  ignore_module_extensions: Vec<String>,
  builtin_modules: Vec<String>,
  tsconfig_config_file: Option<String>,
  tsconfig_references: Option<Vec<String>>,
  alias: Vec<(String, Vec<String>)>,
) -> Result<i32> {
  let tsconfig = match tsconfig_config_file {
    None => None,
    _ => Some(create_tsconfig_option(
      tsconfig_config_file.unwrap(),
      tsconfig_references,
    )),
  };

  let alias_options = create_alias_option(alias);

  let options = ResolveOptions {
    condition_names,
    main_fields,
    extensions,
    tsconfig,
    alias: alias_options,
    ..ResolveOptions::default()
  };
  let mut visited_modules = HashSet::new();
  let mut modules = Vec::new();

  let resolver = Resolver::new(options);

  for file_path in &entry_points {
    let resolved_url = resolver.resolve(&base_path, file_path).unwrap();
    let module_path = diff_paths(resolved_url.full_path(), &base_path).unwrap();

    modules.push(module_path);
  }

  while let Some(dep) = modules.pop() {
    let allocator = Allocator::default();
    let path = PathBuf::from(&base_path).join(&dep);

    let module_extension = path.extension().unwrap().to_str().unwrap();

    if ignore_module_extensions.contains(&module_extension.to_string()) {
      continue;
    }

    let source = match std::fs::read_to_string(PathBuf::from(&base_path).join(&dep)) {
      Ok(source) => source,
      Err(_) => {
        return Err(Error::new(
          GenericFailure,
          format!("Failed to read file: \"{}{}\"", &base_path, &dep.display()),
        ));
      }
    };

    let source_type = SourceType::from_path(PathBuf::from(&base_path).join(&dep)).unwrap();
    let ret = Parser::new(&allocator, &source, source_type).parse();
    let ModuleLexer { imports, .. } = ModuleLexer::new().build(&ret.program);

    visited_modules.insert(dep.to_str().unwrap().to_string());

    for import in imports {
      if import.n.is_none() {
        continue;
      }
      let importee = import.n.unwrap().to_string();

      if builtin_modules.contains(&importee.replace("node:", "")) {
        continue;
      }

      let importer = PathBuf::from(&base_path).join(&dep);
      let parent_path = match importer.parent().unwrap().to_str() {
        Some(path) => path,
        None => {
          return Err(Error::new(
            GenericFailure,
            format!("Failed to get parent path of: \"{}\"", &importer.display()),
          ));
        }
      };
      let resolved_url = match resolver.resolve(parent_path, &importee) {
        Ok(url) => url,
        Err(resolve_error) => {
          return Err(Error::new(
            GenericFailure,
            format!(
              "Failed to resolve importer: \"{}\", importee: \"{}\", message: \"{}\"",
              &importer.display(),
              &importee,
              resolve_error
            ),
          ));
        }
      };

      let path_to_dependency = diff_paths(resolved_url.path(), &base_path).unwrap();
      let path_to_dependency_str = path_to_dependency.to_str().unwrap().to_string();

      if !visited_modules.contains(&path_to_dependency_str) {
        modules.push(path_to_dependency.clone());
      }
    }
  }

  Ok(visited_modules.len() as i32)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_is_bare_module_specifier() {
    assert!(is_bare_module_specifier("@foo"));
    assert!(is_bare_module_specifier("bar"));
    assert!(!is_bare_module_specifier("/baz"));
    assert!(!is_bare_module_specifier("./qux"));
  }
}
