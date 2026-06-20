use crate::error::{NanoDtwError, Result};
use crate::types::{AlignmentInfo, DtwResult, FileFormat};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub trait AlignmentWriter {
    fn write_header(&mut self, references: &[crate::types::ReferenceSequence]) -> Result<()>;
    fn write_alignment(&mut self, alignment: &AlignmentInfo) -> Result<()>;
    fn write_dtw_result(&mut self, result: &DtwResult) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
}

pub struct SamBamWriter {
    writer: Box<dyn Write + Send>,
    format: FileFormat,
    header_written: bool,
    alignments_written: u64,
}

impl SamBamWriter {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let format = FileFormat::from_path(&path);
        let file = File::create(path.as_ref())?;
        let writer = BufWriter::new(file);

        Ok(Self {
            writer: Box::new(writer),
            format,
            header_written: false,
            alignments_written: 0,
        })
    }

    pub fn stdout(format: FileFormat) -> Result<Self> {
        let writer = BufWriter::new(std::io::stdout());
        Ok(Self {
            writer: Box::new(writer),
            format,
            header_written: false,
            alignments_written: 0,
        })
    }

    fn format_sam_record(&self, aln: &AlignmentInfo) -> String {
        let qual = if aln.quality == "*" {
            "*".to_string()
        } else {
            aln.quality.clone()
        };

        let mut optional = String::new();
        if aln.edit_distance > 0 {
            optional.push_str(&format!("\tNM:i:{}", aln.edit_distance));
        }
        if aln.alignment_score != 0 {
            optional.push_str(&format!("\tAS:i:{}", aln.alignment_score));
        }

        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}{}",
            aln.read_id,
            aln.flag,
            aln.reference,
            aln.position,
            aln.mapping_quality,
            aln.cigar,
            "*",
            0,
            0,
            aln.sequence,
            qual,
            optional
        )
    }

    fn dtw_to_alignment(&self, result: &DtwResult) -> AlignmentInfo {
        let cigar = Self::generate_cigar(&result.alignment_path);
        let quality = Self::generate_quality(&result.alignment_path);

        AlignmentInfo {
            read_id: result.read_id.clone(),
            flag: 0,
            reference: result.reference_id.clone(),
            position: result.reference_start as i64 + 1,
            mapping_quality: result.mapping_quality,
            cigar,
            sequence: result.mapped_sequence.clone(),
            quality,
            edit_distance: result.alignment_path.len() as u32,
            alignment_score: -result.normalized_distance as i32,
        }
    }

    fn generate_cigar(path: &[crate::types::DtwPathPoint]) -> String {
        if path.is_empty() {
            return "*".to_string();
        }

        let mut cigar = String::new();
        let mut last_signal_idx = usize::MAX;
        let mut last_ref_idx = usize::MAX;
        let mut current_op = None;
        let mut _current_len = 0usize;

        for point in path {
            let signal_step = if last_signal_idx == usize::MAX {
                1
            } else {
                point.signal_idx - last_signal_idx
            };
            let ref_step = if last_ref_idx == usize::MAX {
                1
            } else {
                point.reference_idx - last_ref_idx
            };

            if signal_step == 1 && ref_step == 1 {
                match current_op {
                    Some(('M', len)) if len < u32::MAX as usize => {
                        _current_len = len + 1;
                        current_op = Some(('M', _current_len));
                    }
                    _ => {
                        if let Some((op, len)) = current_op {
                            let _ = push(&mut cigar, &format!("{}{}", len, op));
                        }
                        current_op = Some(('M', 1));
                    }
                }
            } else if signal_step == 1 && ref_step == 0 {
                match current_op {
                    Some(('I', len)) if len < u32::MAX as usize => {
                        _current_len = len + 1;
                        current_op = Some(('I', _current_len));
                    }
                    _ => {
                        if let Some((op, len)) = current_op {
                            let _ = push(&mut cigar, &format!("{}{}", len, op));
                        }
                        current_op = Some(('I', 1));
                    }
                }
            } else if signal_step == 0 && ref_step == 1 {
                match current_op {
                    Some(('D', len)) if len < u32::MAX as usize => {
                        _current_len = len + 1;
                        current_op = Some(('D', _current_len));
                    }
                    _ => {
                        if let Some((op, len)) = current_op {
                            let _ = push(&mut cigar, &format!("{}{}", len, op));
                        }
                        current_op = Some(('D', 1));
                    }
                }
            }

            last_signal_idx = point.signal_idx;
            last_ref_idx = point.reference_idx;
        }

        if let Some((op, len)) = current_op {
            let _ = push(&mut cigar, &format!("{}{}", len, op));
        }

        if cigar.is_empty() {
            "*".to_string()
        } else {
            cigar
        }
    }

    fn generate_quality(path: &[crate::types::DtwPathPoint]) -> String {
        if path.is_empty() {
            return "*".to_string();
        }

        let mut qual = String::with_capacity(path.len());
        for point in path {
            let q = if point.distance < 1.0 {
                40
            } else if point.distance < 5.0 {
                30
            } else if point.distance < 10.0 {
                20
            } else if point.distance < 20.0 {
                10
            } else {
                5
            };
            let q_char = (q + 33) as u8 as char;
            qual.push(q_char);
        }

        if qual.is_empty() {
            "*".to_string()
        } else {
            qual
        }
    }

    pub fn format(&self) -> FileFormat {
        self.format
    }

    pub fn alignments_written(&self) -> u64 {
        self.alignments_written
    }
}

impl AlignmentWriter for SamBamWriter {
    fn write_header(&mut self, references: &[crate::types::ReferenceSequence]) -> Result<()> {
        match self.format {
            FileFormat::Sam => {
                writeln!(self.writer, "@HD\tVN:1.6\tSO:unsorted")?;
                
                for seq in references {
                    writeln!(
                        self.writer,
                        "@SQ\tSN:{}\tLN:{}",
                        seq.id, seq.length
                    )?;
                }
                
                writeln!(
                    self.writer,
                    "@PG\tID:nanopore-dtw\tPN:nanopore-dtw\tVN:{}",
                    env!("CARGO_PKG_VERSION")
                )?;
            }
            FileFormat::Bam => {
                let mut header = Vec::new();
                writeln!(header, "@HD\tVN:1.6\tSO:unsorted")?;
                for seq in references {
                    writeln!(header, "@SQ\tSN:{}\tLN:{}", seq.id, seq.length)?;
                }
                writeln!(
                    header,
                    "@PG\tID:nanopore-dtw\tPN:nanopore-dtw\tVN:{}",
                    env!("CARGO_PKG_VERSION")
                )?;
                self.writer.write_all(&header)?;
            }
            _ => {
                return Err(NanoDtwError::InvalidFormat(
                    "Invalid output format for SAM/BAM writer".to_string()
                ));
            }
        }

        self.header_written = true;
        Ok(())
    }

    fn write_alignment(&mut self, aln: &AlignmentInfo) -> Result<()> {
        if !self.header_written {
            return Err(NanoDtwError::BamSamError(
                "Header not written before alignments".to_string()
            ));
        }

        match self.format {
            FileFormat::Sam => {
                let line = self.format_sam_record(aln);
                writeln!(self.writer, "{}", line)?;
            }
            FileFormat::Bam => {
                let line = self.format_sam_record(aln);
                writeln!(self.writer, "{}", line)?;
            }
            _ => {
                return Err(NanoDtwError::InvalidFormat(
                    "Invalid output format".to_string()
                ));
            }
        }

        self.alignments_written += 1;
        Ok(())
    }

    fn write_dtw_result(&mut self, result: &DtwResult) -> Result<()> {
        let aln = self.dtw_to_alignment(result);
        self.write_alignment(&aln)
    }

    fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

fn push(cigar: &mut String, s: &str) {
    cigar.push_str(s);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DtwPathPoint;

    #[test]
    fn test_generate_cigar_simple() {
        let path = vec![
            DtwPathPoint { signal_idx: 0, reference_idx: 0, distance: 0.0 },
            DtwPathPoint { signal_idx: 1, reference_idx: 1, distance: 0.0 },
            DtwPathPoint { signal_idx: 2, reference_idx: 2, distance: 0.0 },
        ];
        let cigar = SamBamWriter::generate_cigar(&path);
        assert_eq!(cigar, "3M");
    }

    #[test]
    fn test_generate_cigar_with_indel() {
        let path = vec![
            DtwPathPoint { signal_idx: 0, reference_idx: 0, distance: 0.0 },
            DtwPathPoint { signal_idx: 1, reference_idx: 0, distance: 0.0 },
            DtwPathPoint { signal_idx: 2, reference_idx: 1, distance: 0.0 },
            DtwPathPoint { signal_idx: 2, reference_idx: 2, distance: 0.0 },
        ];
        let cigar = SamBamWriter::generate_cigar(&path);
        assert_eq!(cigar, "1M1I1M1D");
    }
}
