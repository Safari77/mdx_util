use mdx::utils::progress_report::ProgressState;

pub fn print_progress(progress_state: &mut ProgressState) -> bool{
    log::info!("Progress: {}%", if progress_state.total>0 {progress_state.current * 100 / progress_state.total as u64} else {0});
    false
}
