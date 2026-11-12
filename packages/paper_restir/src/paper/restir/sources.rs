//! The ReSTIR investigation's citation index (source-index.json): every
//! section, page, and equation, algorithm line, or figure of the paper the
//! viewer cites, and the works its story cites, written by source_index.py from
//! the PDF.

use lab::utility::sources::PackageIndex;

pub static INDEX: PackageIndex = PackageIndex::new("paper_restir.source-index");
