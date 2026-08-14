use std::fs;
use std::path::Path;

use lexer_generator::api;

#[test]
fn run_all_example_cases() {
    let base = Path::new("examples/cases");
    let mut any_fail = false;
    if let Ok(entries) = fs::read_dir(base) {
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                println!("\n--- Running case: {} ---", name);

                // Try several possible grammar filenames
                let grammar_paths = ["grammar.yapar", "grammar.yalp", "grammar.yapar.yalp"];
                let mut grammar_src = None;
                for g in &grammar_paths {
                    let p = path.join(g);
                    if p.exists() {
                        grammar_src = Some(fs::read_to_string(&p).expect("read grammar"));
                        break;
                    }
                }

                if grammar_src.is_none() {
                    println!("  no grammar file found, skipping");
                    continue;
                }
                let yalp = grammar_src.unwrap();

                let lexer_path = path.join("lexer.yal");
                if !lexer_path.exists() {
                    println!("  no lexer.yal found, skipping");
                    continue;
                }
                let yal = fs::read_to_string(&lexer_path).expect("read lexer");

                let input_path = path.join("input.txt");
                if !input_path.exists() {
                    println!("  no input.txt found, skipping");
                    continue;
                }
                let src = fs::read_to_string(&input_path).expect("read input");

                // Detect expected acceptance from README: if README mentions REJECT/negative, expect rejects.
                let readme_path = path.join("README.md");
                let mut expect_reject = false;
                if readme_path.exists() {
                    if let Ok(rmd) = fs::read_to_string(&readme_path) {
                        let rl = rmd.to_lowercase();
                        if rl.contains("reject") || rl.contains("invalid") || rl.contains("negative") {
                            expect_reject = true;
                        }
                    }
                }

                // Run pipeline per non-empty input line (each line is a separate input case in READMEs)
                for (lineno, line) in src.lines().enumerate() {
                    let line = line.trim();
                    if line.is_empty() { continue; }
                    match api::build_pipeline_response(&yal, &yalp, line, "lalr") {
                        Ok(resp) => {
                            let ok = resp.problems.is_empty() && resp.accepted;
                            if expect_reject {
                                if ok {
                                    any_fail = true;
                                    println!("  [FAIL] case {} line {} expected REJECT but accepted: '{}'", name, lineno+1, line);
                                } else {
                                    println!("  [OK] case {} line {} REJECT as expected: '{}' -> {} problems", name, lineno+1, line, resp.problems.len());
                                }
                            } else {
                                if !ok {
                                    any_fail = true;
                                    println!("  [FAIL] case {} line {} expected ACCEPT but got problems: '{}'", name, lineno+1, line);
                                    for p in &resp.problems {
                                        println!("    - {}", p);
                                    }
                                    println!("    token_map: {}", serde_json::to_string(&resp.token_map).unwrap_or_default());
                                    println!("    trace (last 5): {}", serde_json::to_string(&resp.trace.iter().rev().take(5).collect::<Vec<_>>()).unwrap_or_default());
                                } else {
                                    println!("  [OK] case {} line {} ACCEPT: '{}'", name, lineno+1, line);
                                }
                            }
                        }
                        Err(e) => {
                            any_fail = true;
                            println!("  build_pipeline_response error for {} line {}: {}", name, lineno+1, e);
                        }
                    }
                }
            }
        }
    }
    assert!(!any_fail, "Some example cases produced problems (see output)");
}
