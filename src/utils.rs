use leptos::{
    html::Input,
    NodeRef,
};

pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl IsEmpty for &str {
    fn is_empty(&self) -> bool {
        str::is_empty(self)
    }
}

impl IsEmpty for String {
    fn is_empty(&self) -> bool {
        String::is_empty(&self)
    }
}

pub fn non_empty<T: IsEmpty>(x: T) -> Option<T> {
    if x.is_empty() {
        None
    }
    else {
        Some(x)
    }
}

pub fn get_input_value(node_ref: NodeRef<Input>, clear: bool) -> String {
    let element = node_ref.get_untracked().expect("invalid NodeRef");
    let value = element.value();
    if clear {
        element.set_value("");
    }
    value
}


