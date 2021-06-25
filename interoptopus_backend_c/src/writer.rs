use crate::converter::Converter;
use crate::converter::TypeConverter;
use crate::Config;
use interoptopus::indented;
use interoptopus::lang::c::{CType, CompositeType, Constant, Documentation, EnumType, Field, FnPointerType, Function, OpaqueType, Variant};
use interoptopus::patterns::TypePattern;
use interoptopus::util::sort_types_by_dependencies;
use interoptopus::writer::IndentWriter;
use interoptopus::{Error, Library};

/// Contains all C generators, create sub-trait to customize.
pub trait CWriter {
    /// Returns the user config.
    fn config(&self) -> &Config;

    /// Returns the library to produce bindings for.
    fn library(&self) -> &Library;

    /// Returns the library to produce bindings for.
    fn converter(&self) -> &Converter;

    fn write_custom_defines(&self, w: &mut IndentWriter) -> Result<(), Error> {
        indented!(w, "{}", &self.config().custom_defines)
    }

    fn write_file_header_comments(&self, w: &mut IndentWriter) -> Result<(), Error> {
        indented!(w, "{}", &self.config().file_header_comment)
    }

    fn write_imports(&self, w: &mut IndentWriter) -> Result<(), Error> {
        indented!(w, r#"#include <stdint.h>"#)?;
        indented!(w, r#"#include <stdbool.h>"#)?;

        Ok(())
    }

    fn write_constants(&self, w: &mut IndentWriter) -> Result<(), Error> {
        for constant in self.library().constants() {
            self.write_constant(w, constant)?;
            w.newline()?;
        }

        Ok(())
    }

    fn write_constant(&self, w: &mut IndentWriter, constant: &Constant) -> Result<(), Error> {
        w.indented(|w| write!(w, r#"const "#))?;

        let the_type = match constant.the_type() {
            CType::Primitive(x) => self.converter().type_primitive_to_typename(&x),
            _ => return Err(Error::Null),
        };

        indented!(
            w,
            r#"const {} {} = {};"#,
            the_type,
            constant.name(),
            self.converter().constant_value_to_value(constant.value())
        )
    }

    fn write_functions(&self, w: &mut IndentWriter) -> Result<(), Error> {
        for function in self.library().functions() {
            self.write_function(w, function)?;
        }

        Ok(())
    }

    fn write_function(&self, w: &mut IndentWriter, function: &Function) -> Result<(), Error> {
        self.write_function_declaration(w, function)
    }

    fn write_documentation(&self, w: &mut IndentWriter, documentation: &Documentation) -> Result<(), Error> {
        for line in documentation.lines() {
            indented!(w, r#"/// {}"#, line)?;
        }

        Ok(())
    }

    fn write_function_declaration(&self, w: &mut IndentWriter, function: &Function) -> Result<(), Error> {
        let attr = &self.config().function_attribute;
        let rval = self.converter().type_to_type_specifier(function.signature().rval());
        let name = self.converter().function_name_to_c_name(function);

        let mut params = Vec::new();

        for (_, p) in function.signature().params().iter().enumerate() {
            params.push(format!("{} {}", self.converter().function_parameter_to_csharp_typename(p, function), p.name()));
        }

        indented!(w, r#"{}{} {}({});"#, attr, rval, name, params.join(","))
    }

    fn write_type_definitions(&self, w: &mut IndentWriter) -> Result<(), Error> {
        for the_type in &sort_types_by_dependencies(self.library().ctypes().to_vec()) {
            self.write_type_definition(w, the_type)?;
        }

        Ok(())
    }

    fn write_type_definition(&self, w: &mut IndentWriter, the_type: &CType) -> Result<(), Error> {
        match the_type {
            CType::Primitive(_) => {}
            CType::Enum(e) => {
                self.write_type_definition_enum(w, e)?;
                w.newline()?;
            }
            CType::Opaque(o) => {
                self.write_type_definition_opaque(w, o)?;
                w.newline()?;
            }
            CType::Composite(c) => {
                self.write_type_definition_composite(w, c)?;
                w.newline()?;
            }
            CType::FnPointer(f) => {
                self.write_type_definition_fn_pointer(w, f)?;
                w.newline()?;
            }
            CType::ReadPointer(_) => {}
            CType::ReadWritePointer(_) => {}
            CType::Pattern(p) => match p {
                TypePattern::AsciiPointer => {}
                TypePattern::SuccessEnum(e) => {
                    self.write_type_definition_enum(w, e.the_enum())?;
                    w.newline()?;
                }
                TypePattern::Slice(x) => {
                    self.write_type_definition_composite(w, x)?;
                    w.newline()?;
                }
                TypePattern::Option(x) => {
                    self.write_type_definition_composite(w, x)?;
                    w.newline()?;
                }
            },
        }
        Ok(())
    }

    fn write_type_definition_fn_pointer(&self, w: &mut IndentWriter, the_type: &FnPointerType) -> Result<(), Error> {
        self.write_type_definition_fn_pointer_body(w, the_type)
    }

    fn write_type_definition_fn_pointer_body(&self, w: &mut IndentWriter, the_type: &FnPointerType) -> Result<(), Error> {
        let rval = self.converter().type_to_type_specifier(the_type.signature().rval());
        let name = self.converter().type_fnpointer_to_typename(the_type);

        let mut params = Vec::new();
        for (i, param) in the_type.signature().params().iter().enumerate() {
            params.push(format!("{} x{}", self.converter().type_to_type_specifier(param.the_type()), i));
        }

        indented!(w, "typedef {} (*{})({});", rval, name, params.join(","))
    }

    fn write_type_definition_enum(&self, w: &mut IndentWriter, the_type: &EnumType) -> Result<(), Error> {
        indented!(w, "typedef enum {}", the_type.rust_name())?;
        indented!(w, [_], "{{")?;

        w.indent();

        for variant in the_type.variants() {
            self.write_type_definition_enum_variant(w, variant, the_type)?;
        }

        w.unindent();

        indented!(w, [_], "}} {};", the_type.rust_name())
    }

    fn write_type_definition_enum_variant(&self, w: &mut IndentWriter, variant: &Variant, _the_type: &EnumType) -> Result<(), Error> {
        let variant_name = variant.name();
        let variant_value = variant.value();

        indented!(w, r#"{} = {},"#, variant_name, variant_value)
    }

    fn write_type_definition_opaque(&self, w: &mut IndentWriter, the_type: &OpaqueType) -> Result<(), Error> {
        self.write_type_definition_opaque_body(w, the_type)
    }

    fn write_type_definition_opaque_body(&self, w: &mut IndentWriter, the_type: &OpaqueType) -> Result<(), Error> {
        indented!(w, r#"typedef struct {} {};"#, the_type.rust_name(), the_type.rust_name())
    }

    fn write_type_definition_composite(&self, w: &mut IndentWriter, the_type: &CompositeType) -> Result<(), Error> {
        if the_type.is_empty() {
            // C doesn't allow us writing empty structs.
            indented!(w, r#"typedef struct {} {};"#, the_type.rust_name(), the_type.rust_name())?;
            Ok(())
        } else {
            self.write_type_definition_composite_body(w, the_type)
        }
    }

    fn write_type_definition_composite_body(&self, w: &mut IndentWriter, the_type: &CompositeType) -> Result<(), Error> {
        indented!(w, r#"typedef struct {}"#, the_type.rust_name())?;
        indented!(w, [_], "{{")?;

        w.indent();

        for field in the_type.fields() {
            self.write_type_definition_composite_body_field(w, field, the_type)?;
        }

        w.unindent();

        indented!(w, [_], "}} {};", the_type.rust_name())
    }

    fn write_type_definition_composite_body_field(&self, w: &mut IndentWriter, field: &Field, _the_type: &CompositeType) -> Result<(), Error> {
        let field_name = field.name();
        let type_name = self.converter().type_to_type_specifier(field.the_type());

        indented!(w, r#"{} {};"#, type_name, field_name)
    }

    fn write_ifndef(&self, w: &mut IndentWriter, f: impl FnOnce(&mut IndentWriter) -> Result<(), Error>) -> Result<(), Error> {
        if self.config().directives {
            indented!(w, r#"#ifndef {}"#, self.config().ifndef)?;
            indented!(w, r#"#define {}"#, self.config().ifndef)?;
            w.newline()?;
        }

        f(w)?;

        if self.config().directives {
            w.newline()?;
            indented!(w, r#"#endif /* {} */"#, self.config().ifndef)?;
        }

        Ok(())
    }

    fn write_ifdefcpp(&self, w: &mut IndentWriter, f: impl FnOnce(&mut IndentWriter) -> Result<(), Error>) -> Result<(), Error> {
        if self.config().directives {
            indented!(w, r#"#ifdef __cplusplus"#)?;
            indented!(w, r#"extern "C" {{"#)?;
            indented!(w, r#"#endif"#)?;
            w.newline()?;
        }

        f(w)?;

        if self.config().directives {
            w.newline()?;
            indented!(w, r#"#ifdef __cplusplus"#)?;
            indented!(w, r#"}}"#)?;
            indented!(w, r#"#endif"#)?;
        }
        Ok(())
    }

    fn write_all(&self, w: &mut IndentWriter) -> Result<(), Error> {
        self.write_file_header_comments(w)?;
        w.newline()?;

        self.write_ifndef(w, |w| {
            self.write_ifdefcpp(w, |w| {
                if self.config().imports {
                    self.write_imports(w)?;
                    w.newline()?;
                }

                self.write_custom_defines(w)?;
                w.newline()?;

                self.write_constants(w)?;
                w.newline()?;

                self.write_type_definitions(w)?;
                w.newline()?;

                self.write_functions(w)?;

                Ok(())
            })?;

            Ok(())
        })?;

        Ok(())
    }
}