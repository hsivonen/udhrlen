// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use quick_xml::events::Event;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use unic_normal::StrNormalForm;
use unic_segment::Graphemes;
use unicode_width::UnicodeWidthStr;

#[derive(Debug)]
struct Lang {
    name: String,
    utf8: usize,
    utf16: usize,
    utf32: usize,
    graphemes: usize,
    width: usize,
    code: Option<String>,
    script: Option<String>,
}

fn count(path: &Path, name: String, code: String, script: String) -> std::io::Result<Lang> {
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;

    let mut accu = String::new();
    let mut note = false;
    let mut preamble = false;

    let mut buf = Vec::new();
    let mut xml = quick_xml::Reader::from_str(&content);
    loop {
        match xml.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name() {
                b"preamble" => {
                    assert!(!preamble);
                    preamble = true;
                }
                b"note" => {
                    assert!(!note);
                    note = true;
                }
                _ => {}
            },
            Ok(Event::End(ref e)) => match e.name() {
                b"preamble" => {
                    assert!(preamble);
                    preamble = false;
                }
                b"note" => {
                    assert!(note);
                    note = false;
                }
                _ => {}
            },
            Ok(Event::Text(e)) => {
                if !note && !preamble {
                    let text = e.unescape_and_decode(&xml).unwrap();
                    if !text.as_bytes().iter().all(u8::is_ascii_whitespace) {
                        accu.push_str(&text);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("Error at position {}: {:?}", xml.buffer_position(), e),
        }
    }

    let dhr = accu.nfc().collect::<String>();

    Ok(Lang {
        name: name,
        utf8: dhr.len(),
        utf16: dhr.encode_utf16().count(),
        utf32: dhr.chars().count(),
        graphemes: Graphemes::new(&dhr).count(),
        width: dhr.width(),
        code: Some(code),
        script: Some(script),
    })
}

fn colorize(baseline_result: usize, comparison_result: usize) -> (usize, f64) {
    let (hue, factor) = if baseline_result < comparison_result {
        (0, (baseline_result as f64) / (comparison_result as f64))
    } else {
        (120, (comparison_result as f64) / (baseline_result as f64))
    };
    (hue, (1.0 - factor).powf(0.75) * 100.0)
}

fn deviation_percent(value: usize, median: usize) -> f64 {
    let f_value = value as f64;
    let f_median = median as f64;
    let delta = f_value - f_median;
    (delta / f_median) * 100.0
}

fn print_count(count: usize, median: usize) {
    let (hue, saturation) = colorize(median, count);
    println!(
        "<td style='background-color: hsl({}, {:.*}%, 65%);'>{}</td><td style='background-color: hsl({}, {:.*}%, 65%);'>{:.*}</td>",
        hue,
        6,
        saturation,
        count,
        hue,
        6,
        saturation,
        1,
        deviation_percent(count, median)
    );
}

fn print_lang(
    lang: &Lang,
    median_utf8: usize,
    median_utf16: usize,
    median_utf32: usize,
    median_graphemes: usize,
    median_width: usize,
) {
    println!("<tr>");
    if let Some(code) = &lang.code {
        println!(
            "<th><a href=\"https://www.unicode.org/udhr/d/udhr_{}.html\">{}</a></th>",
            code, lang.name
        );
    } else {
        println!("<th>{}</th>", lang.name);
    }
    print_count(lang.utf8, median_utf8);
    print_count(lang.utf16, median_utf16);
    print_count(lang.utf32, median_utf32);
    print_count(lang.graphemes, median_graphemes);
    print_count(lang.width, median_width);
    println!(
        "<td>{}</td>",
        match &lang.script {
            Some(script) => &script[..],
            None => "",
        }
    );
    println!("</tr>");
}

fn main() -> std::io::Result<()> {
    let mut langs = Vec::new();

    let mut args = std::env::args_os();
    let _ = args.next(); // skip program name

    let dir: PathBuf = Path::new(&args.next().unwrap()).into();
    assert!(dir.is_dir());
    let index_path = dir.join(Path::new("index.xml"));

    let mut index_file = File::open(index_path)?;
    let mut index_text = String::new();
    index_file.read_to_string(&mut index_text)?;

    let mut buf = Vec::new();
    let mut index = quick_xml::Reader::from_str(&index_text);
    loop {
        match index.read_event(&mut buf) {
            Ok(Event::Empty(ref e)) if e.name() == b"udhr" => {
                let mut name = String::new();
                let mut code = String::new();
                let mut script = String::new();
                let mut stage_ok = false;
                for attr in e.attributes() {
                    match attr {
                        Ok(a) => match a.key {
                            b"stage" => {
                                let v = a.unescaped_value().unwrap();
                                stage_ok = (v.len() == 1) && (v[0] == b'4' || v[0] == b'5');
                            }
                            b"f" => {
                                code = a.unescape_and_decode_value(&index).unwrap();
                            }
                            b"n" => {
                                name = a
                                    .unescape_and_decode_value(&index)
                                    .unwrap()
                                    .nfc()
                                    .collect::<String>();
                            }
                            b"iso15924" => {
                                script = a.unescape_and_decode_value(&index).unwrap();
                            }
                            _ => {}
                        },
                        Err(_) => {
                            panic!("Bad attribute");
                        }
                    }
                }
                if stage_ok {
                    assert!(!name.is_empty());
                    assert!(!code.is_empty());
                    let mut file_name = String::from("udhr_");
                    file_name.push_str(&code);
                    file_name.push_str(".xml");
                    langs.push(count(&dir.join(file_name), name, code, script)?);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("Error at position {}: {:?}", index.buffer_position(), e),
        }
    }

    langs.sort_by(|a, b| a.width.cmp(&b.width));
    let median_width = langs[langs.len() / 2].width;
    let min_width = langs[0].width;
    let max_width = langs[langs.len() - 1].width;
    let max2_width = langs[langs.len() - 2].width;


    langs.sort_by(|a, b| a.graphemes.cmp(&b.graphemes));
    let median_graphemes = langs[langs.len() / 2].graphemes;
    let min_graphemes = langs[0].graphemes;
    let max_graphemes = langs[langs.len() - 1].graphemes;
    let max2_graphemes = langs[langs.len() - 2].graphemes;

    langs.sort_by(|a, b| a.utf32.cmp(&b.utf32));
    let median_utf32 = langs[langs.len() / 2].utf32;
    let min_utf32 = langs[0].utf32;
    let max_utf32 = langs[langs.len() - 1].utf32;
    let max2_utf32 = langs[langs.len() - 2].utf32;

    langs.sort_by(|a, b| a.utf16.cmp(&b.utf16));
    let median_utf16 = langs[langs.len() / 2].utf16;
    let min_utf16 = langs[0].utf16;
    let max_utf16 = langs[langs.len() - 1].utf16;
    let max2_utf16 = langs[langs.len() - 2].utf16;

    langs.sort_by(|a, b| a.utf8.cmp(&b.utf8));
    let median_utf8 = langs[langs.len() / 2].utf8;
    let min_utf8 = langs[0].utf8;
    let max_utf8 = langs[langs.len() - 1].utf8;
    let max2_utf8 = langs[langs.len() - 2].utf8;

    println!("<table id=counts>");
    println!("<thead>");
    println!("<tr><th>Name</th><th>UTF-8</th><th>Δ%</th><th>UTF-16</th><th>Δ%</th><th>UTF-32</th><th>Δ%</th><th>EGC</th><th>Δ%</th><th>EAW</th><th>Δ%</th><th>Script</th></tr>");
    println!("</thead>");
    println!("<tbody>");

    let mut total_utf8 = 0usize;
    let mut total_utf16 = 0usize;
    let mut total_utf32 = 0usize;
    let mut total_graphemes = 0usize;
    let mut total_width = 0usize;
    for lang in langs.iter() {
        total_utf8 += lang.utf8;
        total_utf16 += lang.utf16;
        total_utf32 += lang.utf32;
        total_graphemes += lang.graphemes;
        total_width += lang.width;
    }
    let mean_utf8 = total_utf8 / langs.len();
    let mean_utf16 = total_utf16 / langs.len();
    let mean_utf32 = total_utf32 / langs.len();
    let mean_graphemes = total_graphemes / langs.len();
    let mean_width = total_width / langs.len();

    for lang in langs {
        print_lang(
            &lang,
            median_utf8,
            median_utf16,
            median_utf32,
            median_graphemes,
            median_width,
        );
    }

    println!("</tbody>");
    println!("<tfoot>");
    print_lang(
        &Lang {
            name: "Min".to_string(),
            utf8: min_utf8,
            utf16: min_utf16,
            utf32: min_utf32,
            graphemes: min_graphemes,
            width: min_width,
            code: None,
            script: None,
        },
        median_utf8,
        median_utf16,
        median_utf32,
        median_graphemes,
        median_width,
    );
    println!("<tr><th>Median</th><td>{}</td><td></td><td>{}</td><td></td><td>{}</td><td></td><td>{}</td><td></td><td>{}</td><td></td><td></td></tr>", median_utf8, median_utf16, median_utf32, median_graphemes, median_width);
    print_lang(
        &Lang {
            name: "Mean".to_string(),
            utf8: mean_utf8,
            utf16: mean_utf16,
            utf32: mean_utf32,
            graphemes: mean_graphemes,
            width: mean_width,
            code: None,
            script: None,
        },
        median_utf8,
        median_utf16,
        median_utf32,
        median_graphemes,
        median_width,
    );
    print_lang(
        &Lang {
            name: "Max (ignoring outlier)".to_string(),
            utf8: max2_utf8,
            utf16: max2_utf16,
            utf32: max2_utf32,
            graphemes: max2_graphemes,
            width: max2_width,
            code: None,
            script: None,
        },
        median_utf8,
        median_utf16,
        median_utf32,
        median_graphemes,
        median_width,
    );
    print_lang(
        &Lang {
            name: "Max".to_string(),
            utf8: max_utf8,
            utf16: max_utf16,
            utf32: max_utf32,
            graphemes: max_graphemes,
            width: max_width,
            code: None,
            script: None,
        },
        median_utf8,
        median_utf16,
        median_utf32,
        median_graphemes,
        median_width,
    );
    println!("</tfoot>");
    println!("</table>");
    Ok(())
}
