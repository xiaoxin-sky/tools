use rome_diagnostics::v2::Diagnostic;

#[derive(Debug, Diagnostic)]
#[diagnostic(tags = Identifier)]
struct TestDiagnostic {}

fn main() {}
