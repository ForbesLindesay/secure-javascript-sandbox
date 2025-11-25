use std::{
    collections::HashSet,
    fmt::{Display, Write},
};

use swc::atoms::Atom;
use swc_common::{Span, Spanned};
use swc_ecma_ast::*;
use swc_ecma_visit::{Visit, VisitWith};

#[derive(Clone, Debug)]
pub enum ValidModuleExportName {
    Ident(Atom),
    Str(Atom),
}
impl ValidModuleExportName {
    fn default() -> Self {
        ValidModuleExportName::Ident("default".into())
    }
}
impl From<Ident> for ValidModuleExportName {
    fn from(value: Ident) -> Self {
        ValidModuleExportName::Ident(value.sym)
    }
}
impl Display for ValidModuleExportName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidModuleExportName::Ident(ident) => f.write_str(ident.as_str()),
            ValidModuleExportName::Str(lit) => {
                f.write_char('"')?;
                for c in lit.as_str().escape_default() {
                    f.write_char(c)?;
                }
                f.write_char('"')
            }
        }
    }
}

#[derive(Debug)]
pub enum ModuleInputPattern {
    Ident(Atom),
    ObjectPat(Vec<(ValidModuleExportName, Atom)>),
}
impl Display for ModuleInputPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleInputPattern::Ident(ident) => f.write_str(ident.as_str()),
            ModuleInputPattern::ObjectPat(props) => {
                f.write_char('{')?;
                let mut is_first = true;
                for (key, value) in props {
                    if is_first {
                        is_first = false;
                    } else {
                        f.write_char(',')?;
                    }
                    match key {
                        ValidModuleExportName::Ident(ident) if ident == value => {
                            f.write_str(ident.as_str())?;
                        }
                        _ => {
                            write!(f, "{}", key)?;
                            f.write_char(':')?;
                            f.write_str(value.as_str())?;
                        }
                    }
                }
                f.write_char('}')
            }
        }
    }
}

pub struct ModuleImport {
    pub source: Str,
    pub pattern: ModuleInputPattern,
}

pub enum Replacement {
    Whitespace,
    ExportDefaultExpression(Atom),
    ImportFnReference(Atom),
}

pub enum Export {
    ExportNamed {
        exported: ValidModuleExportName,
        local: Atom,
    },
    ExportAll {
        local: Atom,
    },
}

pub struct IdentifierVisitor {
    identifiers: HashSet<u32>,
    default_expression_identifier: Option<Atom>,
    import_fn_identifier: Option<Atom>,
}
impl IdentifierVisitor {
    pub fn new() -> Self {
        Self {
            identifiers: HashSet::new(),
            default_expression_identifier: None,
            import_fn_identifier: None,
        }
    }
    pub fn default_expression_identifier(&mut self) -> Atom {
        if let Some(ident) = &self.default_expression_identifier {
            ident.clone()
        } else {
            let ident = self.gen_identifier();
            self.default_expression_identifier = Some(ident.clone());
            ident
        }
    }
    pub fn import_fn_identifier(&mut self) -> Atom {
        if let Some(ident) = &self.import_fn_identifier {
            ident.clone()
        } else {
            let ident = self.gen_identifier();
            self.import_fn_identifier = Some(ident.clone());
            ident
        }
    }
    pub fn gen_identifier(&mut self) -> Atom {
        let mut i: u32 = 0;
        while self.identifiers.contains(&i) {
            i += 1;
        }
        self.identifiers.insert(i);
        Atom::from(format!("${}", i))
    }
}
impl Visit for IdentifierVisitor {
    fn visit_ident(&mut self, node: &Ident) {
        if node.sym.as_str().starts_with("$") {
            if let Ok(v) = node.sym.as_str()[1..].parse::<u32>() {
                self.identifiers.insert(v);
            }
        }
    }
}

pub struct ModuleVisitor {
    pub import_fn_identifier: Option<Atom>,

    identifiers: IdentifierVisitor,

    /// All statically imported modules and how they are used
    pub imports: Vec<ModuleImport>,

    /// Parts of the module source that need to be replaced with something
    pub replacements: Vec<(Span, Replacement)>,

    /// Exports to be added at the end of the module
    pub exports: Vec<Export>,
}

impl ModuleVisitor {
    pub fn new(identifiers: IdentifierVisitor) -> Self {
        Self {
            import_fn_identifier: None,
            identifiers,
            imports: Vec::new(),
            replacements: Vec::new(),
            exports: Vec::new(),
        }
    }

    fn export_name(&mut self, ident: &Ident) {
        self.exports.push(Export::ExportNamed {
            local: ident.sym.clone(),
            exported: ident.clone().into(),
        });
    }
    fn export_names_from_pat(&mut self, pat: &Pat) {
        match pat {
            Pat::Ident(ident) => {
                self.export_name(&ident.id);
            }
            Pat::Array(array_pat) => {
                for elem in &array_pat.elems {
                    if let Some(elem) = elem {
                        self.export_names_from_pat(elem);
                    }
                }
            }
            Pat::Object(object_pat) => {
                for prop in &object_pat.props {
                    match prop {
                        ObjectPatProp::KeyValue(kv) => {
                            self.export_names_from_pat(&kv.value);
                        }
                        ObjectPatProp::Assign(assign) => {
                            self.export_name(&assign.key);
                        }
                        ObjectPatProp::Rest(rest) => {
                            self.export_names_from_pat(&rest.arg);
                        }
                    }
                }
            }
            Pat::Rest(rest_pat) => {
                self.export_names_from_pat(&rest_pat.arg);
            }
            Pat::Assign(assign_pat) => {
                self.export_names_from_pat(&assign_pat.left);
            }
            Pat::Invalid(_) => todo!(),
            Pat::Expr(_) => todo!(),
        }
    }
    fn export_var_decls(&mut self, var_decls: &[VarDeclarator]) {
        for decl in var_decls {
            self.export_names_from_pat(&decl.name);
        }
    }
}

impl Visit for ModuleVisitor {
    fn visit_import(&mut self, node: &Import) {
        if self.import_fn_identifier.is_none() {
            self.import_fn_identifier = Some(self.identifiers.import_fn_identifier());
        }
        self.replacements.push((
            node.span(),
            Replacement::ImportFnReference(self.identifiers.import_fn_identifier()),
        ));
    }
    fn visit_import_decl(&mut self, node: &ImportDecl) {
        self.imports.push(ModuleImport {
            source: *node.src.clone(),
            pattern: pat_from_import_specifiers(&node.specifiers),
        });
        self.replacements
            .push((node.span(), Replacement::Whitespace));
    }
    fn visit_export_default_expr(&mut self, node: &ExportDefaultExpr) {
        let start = node.span.lo;
        let end = node.expr.span_lo();
        self.replacements.push((
            Span::new(start, end),
            Replacement::ExportDefaultExpression(self.identifiers.default_expression_identifier()),
        ));
        self.exports.push(Export::ExportNamed {
            local: self.identifiers.default_expression_identifier(),
            exported: ValidModuleExportName::default(),
        });
        node.visit_children_with(self);
    }
    fn visit_export_default_decl(&mut self, node: &ExportDefaultDecl) {
        let ident = match &node.decl {
            DefaultDecl::Class(class_expr) => class_expr.ident.as_ref(),
            DefaultDecl::Fn(fn_expr) => fn_expr.ident.as_ref(),
            // Removing TypeScript types is a separate transform
            DefaultDecl::TsInterfaceDecl(_) => return,
        };
        let span = Span::new(node.span.lo, node.decl.span_lo());
        match ident {
            None => {
                self.replacements.push((
                    span,
                    Replacement::ExportDefaultExpression(
                        self.identifiers.default_expression_identifier(),
                    ),
                ));
                self.exports.push(Export::ExportNamed {
                    local: self.identifiers.default_expression_identifier(),
                    exported: ValidModuleExportName::default(),
                });
            }
            Some(ident) => {
                self.replacements.push((span, Replacement::Whitespace));
                self.exports.push(Export::ExportNamed {
                    local: ident.sym.clone(),
                    exported: ValidModuleExportName::default(),
                });
            }
        }
        node.visit_children_with(self);
    }

    fn visit_export_all(&mut self, node: &ExportAll) {
        self.replacements
            .push((node.span(), Replacement::Whitespace));
        let ident = self.identifiers.gen_identifier();
        self.imports.push(ModuleImport {
            source: *node.src.clone(),
            pattern: ModuleInputPattern::Ident(ident.clone()),
        });
        self.exports.push(Export::ExportAll { local: ident });
    }
    fn visit_named_export(&mut self, node: &NamedExport) {
        self.replacements
            .push((node.span(), Replacement::Whitespace));
        if let Some(src) = &node.src {
            let mut imports: Vec<(ValidModuleExportName, Atom)> =
                Vec::with_capacity(node.specifiers.len());
            for specifier in &node.specifiers {
                let ident = self.identifiers.gen_identifier();
                let import: (ValidModuleExportName, Atom) = match specifier {
                    ExportSpecifier::Named(named) => {
                        let imported_name = as_valid_export_name(&named.orig);
                        self.exports.push(Export::ExportNamed {
                            local: ident.clone(),
                            exported: match &named.exported {
                                Some(exported) => as_valid_export_name(exported),
                                None => imported_name.clone(),
                            },
                        });
                        (imported_name, ident)
                    }
                    ExportSpecifier::Default(default) => {
                        self.exports.push(Export::ExportNamed {
                            local: ident.clone(),
                            exported: default.exported.clone().into(),
                        });
                        (ValidModuleExportName::default(), ident)
                    }
                    ExportSpecifier::Namespace(ns) => {
                        self.exports.push(Export::ExportNamed {
                            exported: as_valid_export_name(&ns.name),
                            local: ident.clone(),
                        });
                        self.imports.push(ModuleImport {
                            source: *src.clone(),
                            pattern: ModuleInputPattern::Ident(ident),
                        });
                        return;
                    }
                };
                imports.push(import);
            }
            self.imports.push(ModuleImport {
                source: *src.clone(),
                pattern: ModuleInputPattern::ObjectPat(imports),
            });
        } else {
            for specifier in &node.specifiers {
                let export = match specifier {
                    ExportSpecifier::Named(named) => {
                        let local: Ident = match &named.orig {
                            ModuleExportName::Ident(ident) => ident.clone(),
                            // Cannot have string literal here without a node.src
                            ModuleExportName::Str(_) => unreachable!(),
                        };
                        match &named.exported {
                            Some(exported) => Export::ExportNamed {
                                local: local.sym,
                                exported: as_valid_export_name(exported),
                            },
                            None => Export::ExportNamed {
                                local: local.sym.clone(),
                                exported: local.into(),
                            },
                        }
                    }
                    // Cannot have default or namespace without a node.src
                    ExportSpecifier::Default(_) => unreachable!(),
                    ExportSpecifier::Namespace(_) => unreachable!(),
                };
                self.exports.push(export);
            }
        }
    }
    fn visit_export_decl(&mut self, node: &ExportDecl) {
        self.replacements.push((
            Span::new(node.span_lo(), node.decl.span_lo()),
            Replacement::Whitespace,
        ));
        match &node.decl {
            Decl::Class(c) => {
                self.export_name(&c.ident);
            }
            Decl::Fn(f) => {
                self.export_name(&f.ident);
            }
            Decl::Var(var_decl) => {
                self.export_var_decls(&var_decl.decls);
            }
            Decl::Using(using_decl) => {
                self.export_var_decls(&using_decl.decls);
            }
            // We don't need to export TypeScript types at runtime, so just ignore them here
            Decl::TsInterface(_) => {}
            Decl::TsTypeAlias(_) => {}
            Decl::TsEnum(_) => {}
            Decl::TsModule(_) => {}
        };
        node.visit_children_with(self);
    }
}

fn as_valid_export_name(name: &ModuleExportName) -> ValidModuleExportName {
    match name {
        ModuleExportName::Ident(ident) => ValidModuleExportName::Ident(ident.sym.clone()),
        ModuleExportName::Str(lit) => match lit.value.as_str() {
            Some(value) => ValidModuleExportName::Str(value.into()),
            None => todo!(),
        },
    }
}
fn pat_from_import_specifiers(specifiers: &[ImportSpecifier]) -> ModuleInputPattern {
    let mut props: Vec<(ValidModuleExportName, Atom)> = Vec::with_capacity(specifiers.len());
    for specifier in specifiers {
        let prop = match specifier {
            ImportSpecifier::Named(named) => match &named.imported {
                Some(imported) => (as_valid_export_name(imported), named.local.sym.clone()),
                None => (
                    ValidModuleExportName::Ident(named.local.sym.clone()),
                    named.local.sym.clone(),
                ),
            },
            ImportSpecifier::Default(default) => (
                ValidModuleExportName::Ident("default".into()),
                default.local.sym.clone(),
            ),
            ImportSpecifier::Namespace(ns) => {
                return ModuleInputPattern::Ident(ns.local.sym.clone());
            }
        };
        props.push(prop);
    }

    ModuleInputPattern::ObjectPat(props)
}
