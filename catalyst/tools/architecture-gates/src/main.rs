use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use proc_macro2::Span;
use serde::Deserialize;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{ExprCall, ExprPath, ItemMod, ItemUse, Meta, Path as SynPath, Token, UseTree};
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
struct GatesConfig {
    #[serde(default)]
    application_services: ApplicationServicesConfig,
}

#[derive(Debug, Default, Deserialize)]
struct ApplicationServicesConfig {
    #[serde(default)]
    allow_crate_use_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Report,
    Enforce,
}

#[derive(Debug, Clone)]
struct Finding {
    file_path: String,
    kind: FindingKind,
    line: usize,
    detail: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum FindingKind {
    DirectCrateUse,
    WildcardImport,
    CrateRootCall,
}

impl FindingKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::DirectCrateUse => "direct-crate-use",
            Self::WildcardImport => "wildcard-import",
            Self::CrateRootCall => "crate-root-call",
        }
    }
}

struct ServicePolicyVisitor<'a> {
    relative_path: String,
    allowlisted_paths: &'a HashSet<String>,
    findings: Vec<Finding>,
}

impl<'a> ServicePolicyVisitor<'a> {
    fn new(relative_path: String, allowlisted_paths: &'a HashSet<String>) -> Self {
        Self {
            relative_path,
            allowlisted_paths,
            findings: Vec::new(),
        }
    }

    fn push_finding(&mut self, kind: FindingKind, span: Span, path: String) {
        let line = span.start().line;
        self.findings.push(Finding {
            file_path: self.relative_path.clone(),
            kind,
            line,
            detail: path,
        });
    }

    fn is_crate_path(path: &SynPath) -> bool {
        path.segments
            .first()
            .map(|segment| segment.ident == "crate")
            .unwrap_or(false)
    }

    fn stringify_path(path: &SynPath) -> String {
        path.segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::")
    }

    fn collect_use_tree(
        &mut self,
        tree: &UseTree,
        prefix: &mut Vec<String>,
        span: Span,
    ) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.collect_use_tree(&path.tree, prefix, path.ident.span());
                prefix.pop();
            }
            UseTree::Name(name) => {
                prefix.push(name.ident.to_string());
                self.check_import_path(prefix, name.ident.span());
                prefix.pop();
            }
            UseTree::Rename(rename) => {
                prefix.push(rename.ident.to_string());
                self.check_import_path(prefix, rename.ident.span());
                prefix.pop();
            }
            UseTree::Glob(_) => {
                let mut full = prefix.clone();
                full.push("*".to_string());
                self.push_finding(FindingKind::WildcardImport, span, full.join("::"));
            }
            UseTree::Group(group) => {
                for child in &group.items {
                    self.collect_use_tree(child, prefix, span);
                }
            }
        }
    }

    fn check_import_path(&mut self, segments: &[String], span: Span) {
        if segments.first().map(|segment| segment == "crate").unwrap_or(false) {
            let full = segments.join("::");
            if !self.allowlisted_paths.contains(&full) {
                self.push_finding(FindingKind::DirectCrateUse, span, full);
            }
        }
    }
}

impl<'ast, 'a> Visit<'ast> for ServicePolicyVisitor<'a> {
    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let is_cfg_test_mod = has_cfg_test(&node.attrs);
        if is_cfg_test_mod {
            return;
        }

        visit::visit_item_mod(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        let mut prefix = Vec::new();
        self.collect_use_tree(&node.tree, &mut prefix, node.span());
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let syn::Expr::Path(ExprPath { path, .. }) = &*node.func {
            if Self::is_crate_path(path) {
                let path_str = Self::stringify_path(path);
                self.push_finding(FindingKind::CrateRootCall, path.span(), path_str);
            }
        }
        visit::visit_expr_call(self, node);
    }
}

fn parse_args() -> Result<(PathBuf, PathBuf, Mode)> {
    let mut root: Option<PathBuf> = None;
    let mut config_path: Option<PathBuf> = None;
    let mut mode = Mode::Report;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                let value = args.next().context("missing value for --root")?;
                root = Some(PathBuf::from(value));
            }
            "--config" => {
                let value = args.next().context("missing value for --config")?;
                config_path = Some(PathBuf::from(value));
            }
            "--mode" => {
                let value = args.next().context("missing value for --mode")?;
                mode = match value.as_str() {
                    "report" => Mode::Report,
                    "enforce" => Mode::Enforce,
                    _ => bail!("unsupported mode: {value}"),
                };
            }
            _ => bail!("unsupported argument: {arg}"),
        }
    }

    let root = root.unwrap_or(env::current_dir().context("unable to read current directory")?);
    let config_path = config_path.unwrap_or_else(|| root.join("scripts").join("architecture-gates.json"));
    Ok((root, config_path, mode))
}

fn has_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        match &attr.meta {
            Meta::List(list) => {
                let parsed = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated);
                if let Ok(entries) = parsed {
                    return entries
                        .iter()
                        .any(|entry| matches!(entry, Meta::Path(path) if path.is_ident("test")));
                }
                list.tokens.to_string().replace(' ', "") == "test"
            }
            _ => false,
        }
    })
}

fn load_config(path: &Path) -> Result<GatesConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("unable to read gate config: {}", path.display()))?;
    let config = serde_json::from_str::<GatesConfig>(&content)
        .with_context(|| format!("invalid gate config JSON: {}", path.display()))?;
    Ok(config)
}

fn collect_service_files(root: &Path) -> Vec<PathBuf> {
    let mut files = WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().map(|ext| ext == "rs").unwrap_or(false))
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();

    files.sort();
    files
}

fn run_checker(root: &Path, config: &GatesConfig) -> Result<Vec<Finding>> {
    let services_root = root
        .join("src-tauri")
        .join("src")
        .join("application")
        .join("services");
    let files = collect_service_files(&services_root);

    let allowlisted_paths = config
        .application_services
        .allow_crate_use_paths
        .iter()
        .cloned()
        .collect::<HashSet<_>>();

    let mut findings = Vec::new();

    for file in files {
        let content = fs::read_to_string(&file)
            .with_context(|| format!("unable to read service file: {}", file.display()))?;
        let parsed = syn::parse_file(&content)
            .with_context(|| format!("unable to parse rust file: {}", file.display()))?;

        let relative = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");

        let mut visitor = ServicePolicyVisitor::new(relative, &allowlisted_paths);
        visitor.visit_file(&parsed);
        findings.extend(visitor.findings);
    }

    findings.sort_by(|left, right| {
        (left.kind, &left.file_path, left.line, &left.detail)
            .cmp(&(right.kind, &right.file_path, right.line, &right.detail))
    });

    Ok(findings)
}

fn print_findings(findings: &[Finding]) {
    if findings.is_empty() {
        println!("AST architecture report: no findings in application services.");
        return;
    }

    println!("AST architecture report: found {} item(s).", findings.len());
    println!("Policy is report-only in this phase; existing regex guards remain authoritative.");
    for finding in findings {
        println!(
            "- {}:{} [{}] {}",
            finding.file_path,
            finding.line,
            finding.kind.as_str(),
            finding.detail
        );
    }
}

fn main() -> Result<()> {
    let (root, config_path, mode) = parse_args()?;
    let config = load_config(&config_path)?;
    let findings = run_checker(&root, &config)?;

    print_findings(&findings);

    if matches!(mode, Mode::Enforce) && !findings.is_empty() {
        bail!("AST architecture enforcement failed with {} finding(s)", findings.len());
    }

    Ok(())
}
