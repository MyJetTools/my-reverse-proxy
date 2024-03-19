use std::collections::HashMap;

use rust_extensions::StrOrString;

pub fn populate_variable<'s>(
    src: &'s str,
    variables: &Option<HashMap<String, String>>,
) -> StrOrString<'s> {
    let index = src.find("${");

    if index.is_none() {
        return src.into();
    }

    let mut value = replace_variable(src, index.unwrap(), variables);

    loop {
        let index = value.find("${");
        if index.is_none() {
            break;
        }

        value = replace_variable(src, index.unwrap(), variables);
    }

    value.into()
}

fn replace_variable(
    src: &str,
    index: usize,
    variables: &Option<HashMap<String, String>>,
) -> String {
    let end_index = src[index..].find("}");

    if end_index.is_none() {
        return src.to_string();
    }

    let end_index = end_index.unwrap() + index;

    let variable_name = &src[index + 2..end_index];

    if variables.is_none() {
        println!(
            "There is a variable: {} but no variables dictionary",
            variable_name
        );
    }

    let value = variables.as_ref().unwrap().get(variable_name);

    if value.is_none() {
        panic!("Variable with name: '{}' is not defined", variable_name)
    }

    let value = value.unwrap();

    let mut result = String::new();

    result.push_str(&src[..index]);
    result.push_str(value);
    result.push_str(&src[end_index + 1..]);

    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    #[test]
    fn test_replace_one_variable() {
        let mut variables = HashMap::new();
        variables.insert("MyValue".to_string(), "ssh:12.0.0.0".to_string());

        let src = "${MyValue}->10.0.0.5:8080";

        let result = super::populate_variable(src, &Some(variables));

        assert_eq!(result.as_str(), "ssh:12.0.0.0->10.0.0.5:8080");
    }
}
