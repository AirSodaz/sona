pub fn should_exit_before_app() -> bool {
    std::env::var_os("SONA_TEST_EXIT_BEFORE_APP").is_some()
}
