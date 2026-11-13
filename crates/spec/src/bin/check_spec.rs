//! 文档检查，一次跑完。钩子与 CI 都只调这一条。
//! 参考：docs/Kernel/Engineering.md（接口）。

use std::process::ExitCode;

use spec::limits::{Verdict, check};
use spec::tokens;

fn main() -> ExitCode {
    let root = match spec::repo_root() {
        Ok(root) => root,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    let mut failed = false;
    match spec::docs(&root) {
        Ok(found) => {
            println!("block：{} 个，名字唯一", found.len());
            match spec::expand(&found) {
                Ok(_) => println!("$block：全部展开得了"),
                Err(errors) => {
                    for error in errors {
                        eprintln!("{error}");
                    }
                    failed = true;
                }
            }
        }
        Err(error) => {
            eprintln!("{error}");
            failed = true;
        }
    }
    match spec::check_glossary(&root) {
        Ok(()) => println!("名词表：名字唯一"),
        Err(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            failed = true;
        }
    }
    match spec::check_test_markers(&root) {
        Ok(()) => println!("测试标记：全部找得到"),
        Err(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            failed = true;
        }
    }
    match spec::check_references(&root) {
        Ok(()) => println!("参考：文档与 Rust 模块注释全部可解析"),
        Err(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            failed = true;
        }
    }
    let mut over_healthy = Vec::new();
    match check(&root) {
        Ok(measures) => {
            let mut warned = 0;
            for measure in &measures {
                match measure.verdict {
                    Verdict::OverHealthy => {
                        warned += 1;
                        over_healthy.push((measure.path.clone(), measure.lines));
                        eprintln!(
                            "警告：{} {} 行，超过健康值；可以考虑拆开",
                            measure.path, measure.lines
                        );
                    }
                    Verdict::Small => {
                        eprintln!(
                            "{} {} 行，不到最小值；应当合并进相邻的部分",
                            measure.path, measure.lines
                        );
                        failed = true;
                    }
                    Verdict::OverDanger => {
                        eprintln!(
                            "{} {} 行，超过危险值；应当拆成文件夹，由 README.md 承担核心",
                            measure.path, measure.lines
                        );
                        failed = true;
                    }
                    Verdict::Healthy => {}
                }
            }
            println!("篇幅：{} 个文件，{} 个警告", measures.len(), warned);
        }
        Err(message) => {
            eprintln!("{message}");
            failed = true;
        }
    }

    let mut measured = None;
    match tokens::measure(&root) {
        Ok(measure) => {
            println!("token：{}", measure.summary());
            if measure.over() {
                eprintln!("正文超过上下文的上限；删去重复，或把内容移出正文");
                failed = true;
            } else {
                measured = Some(measure);
            }
        }
        Err(message) => {
            eprintln!("{message}");
            failed = true;
        }
    }
    // 全部通过才写：失败的提交不会带着改过的 AGENTS.md。
    if !failed {
        if let Some(measure) = measured {
            match tokens::stamp(&root, &measure, &over_healthy) {
                Ok(tokens::Stamp::Unchanged) => println!("token：AGENTS.md 里的实测值没变"),
                Ok(tokens::Stamp::Written { synced: true }) => {
                    println!("token：实测值已写进 AGENTS.md 的 context.usage")
                }
                Ok(tokens::Stamp::Written { synced: false }) => println!(
                    "token：实测值已写进即将提交的 AGENTS.md；工作区的那份有没提交的改动，没动它"
                ),
                Err(message) => {
                    eprintln!("{message}");
                    failed = true;
                }
            }
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
