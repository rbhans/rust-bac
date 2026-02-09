#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtomicReadFileResult {
    Stream {
        end_of_file: bool,
        file_start_position: i32,
        file_data: Vec<u8>,
    },
    Record {
        end_of_file: bool,
        file_start_record: i32,
        returned_record_count: u32,
        file_record_data: Vec<Vec<u8>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicWriteFileResult {
    Stream { file_start_position: i32 },
    Record { file_start_record: i32 },
}
