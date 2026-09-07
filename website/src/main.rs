fn main() -> anyhow::Result<()> {
    fission::site::build_from_cli(coil_website::site())
}
