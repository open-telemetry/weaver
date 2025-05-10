use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::PathBuf,
    str::FromStr,
};

use itertools::Itertools;
use schemars::{
    schema::{
        InstanceType, Metadata, ObjectValidation, RootSchema, Schema, SchemaObject, SingleOrVec,
    },
    schema_for,
};
use weaver_forge::registry::ResolvedRegistry;

// TODO - This should actually be a link to the type...
fn type_string_ref(r: &str) -> String {
    if r.starts_with("#/definitions/") {
        let rr = r.chars().skip(14).collect::<String>();
        return format!("[`{}`](#{})", &rr, &rr);
    }
    r.to_owned()
}

fn type_string_svs(os: &SingleOrVec<Schema>) -> String {
    match os {
        SingleOrVec::Single(s) => type_string(&s.clone().into_object()),
        SingleOrVec::Vec(sts) => {
            return sts
                .iter()
                .map(|t| type_string(&t.clone().into_object()))
                .join(" or ")
        }
    }
}

fn type_string_it(tpe: &InstanceType) -> String {
    match tpe {
        schemars::schema::InstanceType::Null => {
            return "`null`".to_owned();
        }
        schemars::schema::InstanceType::Boolean => {
            return "`boolean`".to_owned();
        }
        schemars::schema::InstanceType::Object => {
            return "`Object`".to_owned();
        }
        schemars::schema::InstanceType::Array => {
            return "`Object`[]".to_owned();
        }
        schemars::schema::InstanceType::Number => {
            return "`double`".to_owned();
        }
        schemars::schema::InstanceType::String => {
            return "`String`".to_owned();
        }
        schemars::schema::InstanceType::Integer => {
            return "`int`".to_owned();
        }
    }
}

fn type_string_svi(os: &SingleOrVec<InstanceType>) -> String {
    match os {
        SingleOrVec::Single(s) => type_string_it(s),
        SingleOrVec::Vec(sts) => return sts.iter().map(|t| type_string_it(t)).join(" or "),
    }
}

fn type_string(os: &SchemaObject) -> String {
    if let Some(r) = os.reference.as_ref() {
        return type_string_ref(r);
    }
    if let Some(tpe) = os.instance_type.as_ref() {
        match tpe {
            SingleOrVec::Single(st) => {
                match st.as_ref() {
                    schemars::schema::InstanceType::Null => {
                        return "`null`".to_owned();
                    }
                    schemars::schema::InstanceType::Boolean => {
                        return "`boolean`".to_owned();
                    }
                    schemars::schema::InstanceType::Object => {
                        // TODO - pull in object definition locally?
                        // let ov = os.object.as_ref().unwrap().as_ref();
                        return "`Object`".to_owned();
                    }
                    schemars::schema::InstanceType::Array => {
                        let av = os.array.as_ref().unwrap().as_ref();
                        if let Some(iv) = av.items.as_ref() {
                            // TODO - unwrap this.
                            return format!("{}[]", type_string_svs(iv));
                        } else {
                            return "`Object`[]".to_owned();
                        }
                    }
                    schemars::schema::InstanceType::Number => {
                        return "`double`".to_owned();
                    }
                    schemars::schema::InstanceType::String => {
                        return "`String`".to_owned();
                    }
                    schemars::schema::InstanceType::Integer => {
                        return "`int`".to_owned();
                    }
                }
            }
            SingleOrVec::Vec(sts) => return sts.iter().map(|t| format!("{:?}", t)).join(" or "),
        }
    }
    if let Some(ss) = os.subschemas.as_ref() {
        // Check for enum value
        if let Some(ao) = ss.all_of.as_ref() {
            return ao
                .iter()
                .map(|s| type_string(&s.to_owned().into_object()))
                .join(" and ");
        } else if let Some(ao) = ss.any_of.as_ref() {
            return ao
                .iter()
                .map(|s| type_string(&s.to_owned().into_object()))
                .join(" or ");
        }
    }
    format!("{:?}", os)
    // "<unknown type>".to_owned()
}

// Prints a table of fields for an ObjectValidation.
fn print_object_field_table<O: Write>(out: &mut O, o: &ObjectValidation) -> anyhow::Result<()> {
    writeln!(out, "| field | type | description |")?;
    writeln!(out, "| --- | --- | --- |")?;
    for (field, v) in o.properties.iter() {
        let os = v.clone().into_object();
        write!(out, "| {} | {} |", field, type_string(&os))?;

        if let Some(desc) = os.metadata.as_ref().and_then(|md| md.description.as_ref()) {
            write!(out, " {} |", desc)?;
        } else {
            write!(out, " |")?;
        }
        writeln!(out, "")?;
    }
    Ok(())
}

fn print_raw_schema<O: Write>(out: &mut O, schema: &SchemaObject) -> anyhow::Result<()> {
    if let Some(obj) = schema.object.as_ref() {
        write!(out, "**An Object:**")?;
        if let Some(desc) = schema
            .metadata
            .as_ref()
            .and_then(|f| f.description.as_ref())
        {
            writeln!(out, " {}", desc)?;
        } else {
            writeln!(out, "")?;
        }
        writeln!(out, "")?;
        print_object_field_table(out, &obj)?;
        writeln!(out, "")?;
    } else if let Some(ss) = schema.enum_values.as_ref() {
        // Handle enumerated values.  These are encoded oddly.
        write!(out, "- ")?;
        for v in ss.iter() {
            write!(out, "`{}` ", v)?;
        }
        if let Some(desc) = schema
            .metadata
            .as_ref()
            .and_then(|f| f.description.as_ref())
        {
            writeln!(out, ": {}", desc)?;
        } else {
            writeln!(out, "")?;
        }
    // This should be near the bottom, so we handle instance types AFTER better docs are handled.
    } else if let Some(it) = schema.instance_type.as_ref() {
        let tpe = type_string_svi(it);
        write!(out, "- {}", &tpe)?;
        if let Some(desc) = schema
            .metadata
            .as_ref()
            .and_then(|f| f.description.as_ref())
        {
            writeln!(out, ": {}", desc)?;
        } else {
            writeln!(out, "")?;
        }
    // Next we cover deferring to a subschema validation of a reference.
    } else if let Some(it) = schema
        .subschemas
        .as_ref()
        .and_then(|ss| ss.all_of.as_ref())
        .and_then(|s| s.iter().next())
        .and_then(|s| match s {
            Schema::Bool(_) => None,
            Schema::Object(obj) => obj.reference.as_ref(),
        })
    {
        let tpe = type_string_ref(it);
        write!(out, "- {}", &tpe)?;
        if let Some(desc) = schema
            .metadata
            .as_ref()
            .and_then(|f| f.description.as_ref())
        {
            writeln!(out, ": {}", desc)?;
        } else {
            writeln!(out, "")?;
        }
    } else {
        writeln!(out, "**An Unknown:**")?;
        writeln!(out, "{:?}", schema)?;
        writeln!(out, "")?;
    }
    Ok(())
}

fn print_schema_for_type<O: Write>(
    out: &mut O,
    type_name: &str,
    schema: &SchemaObject,
) -> anyhow::Result<()> {
    let empty_md = Box::new(Metadata::default());
    let md = schema.metadata.as_ref().unwrap_or(&empty_md);
    writeln!(out, "## {}", type_name)?;
    writeln!(out, "")?;
    if let Some(desc) = md.description.as_ref() {
        writeln!(out, "{}", desc)?;
        writeln!(out, "")?;
    }
    if let Some(o) = schema.object.as_ref() {
        print_object_field_table(out, o.as_ref())?;
        writeln!(out, "")?;
    } else if let Some(en) = schema.enum_values.as_ref() {
        writeln!(out, "Enum values: ")?;
        writeln!(out, "")?;
        for v in en.iter() {
            writeln!(out, "- {}", v)?;
        }
        writeln!(out, "")?;
    } else if let Some(ss) = schema.subschemas.as_ref() {
        // TODO - handle possible sub-schemas.
        if let Some(one) = ss.one_of.as_ref() {
            writeln!(out, "It will have one of the following values: ")?;
            writeln!(out, "")?;
            for i in one.iter() {
                print_raw_schema(out, &i.to_owned().into_object())?;
            }
            writeln!(out, "")?;
        } else if let Some(one) = ss.any_of.as_ref() {
            writeln!(out, "It will have any of the following values: ")?;
            writeln!(out, "")?;
            for i in one.iter() {
                print_raw_schema(out, &i.to_owned().into_object())?;
            }
            writeln!(out, "")?;
        } else {
            // TODO - flesh this out
            writeln!(out, "todo: {:?}", ss)?;
            writeln!(out, "")?;
        }
    } else if let Some(it) = schema.instance_type.as_ref() {
        writeln!(out, "Type: {}", type_string_svi(it))?;
        writeln!(out, "")?;
    } else {
        writeln!(out, "unknown type: {:?}", schema)?;
        // writeln!(out, "unknown type")?;
        writeln!(out, "")?;
    }
    Ok(())
}

// Dump schema documents for weaver_forge.
fn create_schema_docs() -> anyhow::Result<()> {
    let schema = schema_for!(ResolvedRegistry);
    let docs = PathBuf::from_str("docs/data")?;
    std::fs::create_dir_all(docs.clone())?;
    let registry_model_file = docs.clone().join("registry.md");
    let f = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(registry_model_file.clone())?;

    // Find all definitions and put them in this document.
    let mut out = BufWriter::new(f);
    writeln!(out, "# Template DataModels")?;
    writeln!(out, "")?;

    let mut schemas = BTreeMap::new();
    let root_name = schema
        .schema
        .metadata
        .as_ref()
        .map(|md| md.title.as_ref())
        .flatten()
        .unwrap();
    println!("Found schema: {}", root_name);
    schemas.insert(root_name, schema.schema.clone());
    for (k, s) in schema.definitions.iter() {
        println!("Found schema: {}", k);
        schemas.insert(k, s.clone().into_object());
    }
    for (name, s) in schemas.into_iter() {
        print_schema_for_type(&mut out, name, &s)?;
    }
    println!("Created {}", registry_model_file.canonicalize()?.display());
    Ok(())
}

fn main() -> anyhow::Result<()> {
    create_schema_docs()?;
    Ok(())
}
