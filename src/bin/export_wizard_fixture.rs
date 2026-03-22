use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use component_llm_openai::{
    component_describe_ir, encode_cbor_for_tests, fixture_apply_config_cbor, fixture_key,
    fixture_qa_spec_cbor,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut reference = None;
    let mut out_dir = None;
    let mut default_answers = None;
    let mut setup_answers = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--reference" => reference = args.next(),
            "--out" => out_dir = args.next(),
            "--default-answers" => default_answers = args.next(),
            "--setup-answers" => setup_answers = args.next(),
            other => {
                return Err(io::Error::other(format!("unexpected argument `{other}`")).into());
            }
        }
    }

    let reference = reference.ok_or_else(|| io::Error::other("missing --reference"))?;
    let out_dir = PathBuf::from(out_dir.ok_or_else(|| io::Error::other("missing --out"))?);
    let default_answers = read_json_object(Path::new(
        default_answers
            .as_deref()
            .ok_or_else(|| io::Error::other("missing --default-answers"))?,
    ))?;
    let setup_answers = read_json_object(Path::new(
        setup_answers
            .as_deref()
            .ok_or_else(|| io::Error::other("missing --setup-answers"))?,
    ))?;

    fs::create_dir_all(&out_dir)?;

    let key = fixture_key(&reference);
    let describe = component_describe_ir();
    fs::write(
        out_dir.join(format!("{key}.describe.cbor")),
        encode_cbor_for_tests(&describe)?,
    )?;
    fs::write(
        out_dir.join(format!("{key}.qa-default.cbor")),
        fixture_qa_spec_cbor("default")?,
    )?;
    fs::write(
        out_dir.join(format!("{key}.qa-setup.cbor")),
        fixture_qa_spec_cbor("setup")?,
    )?;
    fs::write(
        out_dir.join(format!("{key}.apply-default-config.cbor")),
        fixture_apply_config_cbor("default", &default_answers)?,
    )?;
    fs::write(
        out_dir.join(format!("{key}.apply-setup-config.cbor")),
        fixture_apply_config_cbor("setup", &setup_answers)?,
    )?;
    fs::write(out_dir.join(format!("{key}.abi")), "0.6.0\n")?;

    Ok(())
}

fn read_json_object(path: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    if !value.is_object() {
        return Err(
            io::Error::other(format!("{} must contain a JSON object", path.display())).into(),
        );
    }
    Ok(value)
}
