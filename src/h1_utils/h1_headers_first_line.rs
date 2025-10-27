use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

pub struct H1HeadersFirstLine<'s> {
    pub data: &'s [u8],
}

impl<'s> H1HeadersFirstLine<'s> {
    pub fn get_path(&self) -> &str {
        let index_start = self.data.find_byte_pos(b' ', 0).unwrap() + 1;
        let index_end = self.data.find_byte_pos(b' ', index_start).unwrap();

        std::str::from_utf8(&self.data[index_start..index_end]).unwrap()
    }

    pub fn get_verb_and_path(&self) -> (&str, &str) {
        let index_start = self.data.find_byte_pos(b' ', 0).unwrap();
        let index_end = self.data.find_byte_pos(b' ', index_start + 1).unwrap();

        let verb = std::str::from_utf8(&self.data[..index_start]).unwrap();
        let path = std::str::from_utf8(&self.data[index_start + 1..index_end]).unwrap();

        (verb, path)
    }
}
