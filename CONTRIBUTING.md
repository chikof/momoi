# Contributing to Momoi

Thank you for considering contributing to the Momoi! This document provides guidelines and instructions for contributing.

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Getting Started](#getting-started)
3. [Development Setup](#development-setup)
4. [Making Changes](#making-changes)
5. [Testing](#testing)
6. [Submitting Changes](#submitting-changes)
7. [Coding Standards](#coding-standards)
8. [Documentation](#documentation)
9. [Community](#community)

---

## Code of Conduct

This project adheres to a code of conduct that fosters an open and welcoming environment:

- **Be respectful**: Treat everyone with respect and kindness
- **Be inclusive**: Welcome people of all backgrounds and identities
- **Be constructive**: Provide helpful, constructive feedback
- **Be professional**: Keep discussions focused and on-topic
- **Be patient**: Remember that everyone is learning

Unacceptable behavior will not be tolerated. Report issues to the project maintainers.

---

## Getting Started

### Ways to Contribute

- **Report bugs**: Found an issue? Let us know!
- **Suggest features**: Have an idea? We'd love to hear it!
- **Write code**: Fix bugs or implement features
- **Improve documentation**: Fix typos, clarify instructions, add examples
- **Review pull requests**: Help review others' contributions
- **Answer questions**: Help users in Discussions or Issues

### Before You Start

1. **Check existing issues**: Someone might already be working on it
2. **Read the docs**: Familiarize yourself with the project
3. **Discuss major changes**: Open an issue first for significant changes

---

## Development Setup

### Prerequisites

- **NixOS** or **Nix package manager** (recommended)
- **Rust** 1.70+ (via Nix or rustup)
- **Wayland compositor** (Sway, Hyprland, etc.) for testing
- **Vulkan drivers** for GPU feature testing

### Setup Steps

1. **Clone the repository**:

   ```bash
   git clone https://github.com/chikof/momoi.git
   cd momoi
   ```

2. **Enter the development environment**:

   ```bash
   nix develop
   # Or if using direnv:
   direnv allow
   ```

3. **Build the project**:

   ```bash
   cargo build --all-features
   ```

4. **Run tests**:

   ```bash
   cargo test --all
   ```

5. **Start the daemon** (for testing):
   ```bash
   RUST_LOG=debug cargo run --bin momoi
   ```

---

## Making Changes

### Workflow

1. **Create a branch** from `main`:

   ```bash
   git checkout -b feature/your-feature-name
   # or
   git checkout -b fix/bug-description
   ```

2. **Make your changes**:
   - Write clear, focused commits
   - Follow the coding standards (see below)
   - Add tests for new functionality
   - Update documentation as needed

3. **Test your changes**:

   ```bash
   cargo test --all
   cargo clippy --all-features
   cargo fmt --all -- --check
   ```

4. **Commit your changes**:
   ```bash
   git add .
   git commit -m "feat: add new shader parameter validation"
   ```

### Commit Message Format

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types**:

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

**Examples**:

```
feat(gpu): add support for custom shader parameters
fix(ipc): resolve connection timeout on slow systems
docs(readme): update installation instructions for Arch Linux
test(config): add tests for shader preset parsing
```

---

## Testing

### Running Tests

```bash
# All tests
cargo test --all

# Specific test suite
cargo test --package common
cargo test --bin momoi config::tests

# Integration tests
cargo test --test ipc_integration

# With logging
RUST_LOG=debug cargo test -- --nocapture
```

### Writing Tests

- **Unit tests**: Add `#[cfg(test)]` modules in source files
- **Integration tests**: Add files to `daemon/tests/`
- **Test coverage**: Aim for >80% coverage of critical paths
- **Test naming**: Use descriptive names like `test_shader_params_validation`

**Example unit test**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        let (r, g, b) = ShaderParams::parse_color("FF0000").unwrap();
        assert_eq!(r, 1.0);
        assert_eq!(g, 0.0);
        assert_eq!(b, 0.0);
    }
}
```

### Manual Testing

For features requiring runtime testing:

1. Start the daemon: `cargo run --bin momoi`
2. Test with client: `cargo run --bin wwctl -- set test.png`
3. Check logs: `tail -f /tmp/wwdaemon.log`
4. Monitor resources: `cargo run --bin wwctl -- resources`

---

## Submitting Changes

### Pull Request Process

1. **Push your branch**:

   ```bash
   git push origin feature/your-feature-name
   ```

2. **Open a pull request** on GitHub:
   - Provide a clear title and description
   - Reference related issues: `Fixes #123` or `Closes #456`
   - Describe what changed and why
   - Include screenshots for UI changes
   - List breaking changes if any

3. **Address review feedback**:
   - Make requested changes
   - Push updates to the same branch
   - Respond to comments

4. **Wait for approval**:
   - At least one maintainer approval required
   - CI checks must pass
   - No merge conflicts

### Pull Request Template

```markdown
## Description

Brief description of changes.

## Related Issues

Fixes #123

## Changes Made

- Added X feature
- Fixed Y bug
- Updated Z documentation

## Testing

- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing performed
- [ ] Documentation updated

## Screenshots

(If applicable)

## Breaking Changes

(If any)

## Checklist

- [ ] Code follows style guidelines
- [ ] Tests pass locally
- [ ] Documentation updated
- [ ] Changelog updated (for significant changes)
```

---

## Coding Standards

### Rust Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting (enforced by CI)
- Use `cargo clippy` and fix warnings (enforced by CI)
- Maximum line length: 100 characters (soft limit)

### Code Organization

- **One module per file** for clarity
- **Public APIs**: Well-documented with rustdoc
- **Error handling**: Use `Result` and `anyhow`/`thiserror`
- **Logging**: Use `log` crate (debug, info, warn, error)

### Documentation

- **Public functions**: Document with `///` rustdoc comments
- **Modules**: Add module-level documentation with `//!`
- **Examples**: Include examples in documentation
- **Panics**: Document when functions can panic
- **Safety**: Document `unsafe` code thoroughly

**Example**:

````rust
/// Parses a hex color string to RGB float values.
///
/// # Arguments
///
/// * `hex` - Hex color string (e.g., "FF0000" or "#FF0000")
///
/// # Returns
///
/// Tuple of (r, g, b) where each component is 0.0-1.0, or None if invalid.
///
/// # Examples
///
/// ```
/// use common::ShaderParams;
///
/// let (r, g, b) = ShaderParams::parse_color("FF0000").unwrap();
/// assert_eq!(r, 1.0);
/// ```
pub fn parse_color(hex: &str) -> Option<(f32, f32, f32)> {
    // Implementation
}
````

### Performance

- **Avoid allocations** in hot paths
- **Profile before optimizing**: Measure first
- **Use benchmarks** for performance-critical code
- **Document complexity**: Big-O notation for algorithms

### Error Handling

- **Use Result**: Don't panic in library code
- **Provide context**: Use `anyhow::Context` to add error context
- **Specific errors**: Use `thiserror` for public error types
- **Don't unwrap**: Except in tests or when provably safe

---

## Documentation

### Types of Documentation

1. **Code documentation** (rustdoc)
   - Generate with: `cargo doc --open`
   - Document all public APIs

2. **User documentation** (Markdown files)
   - README.md - Project overview
   - CONFIGURATION.md - Configuration guide
   - TROUBLESHOOTING.md - Common issues
   - TESTING.md - Testing guide

### Documentation Guidelines

- **Be clear and concise**: Avoid jargon
- **Provide examples**: Show, don't just tell
- **Keep it updated**: Update docs with code changes
- **Check spelling/grammar**: Use a spell checker
- **Test examples**: Ensure code examples work

---

## Community

### Communication Channels

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: Q&A and general discussion
- **Pull Requests**: Code review and collaboration

### Getting Help

- **Read the docs first**: Check README and documentation
- **Search existing issues**: Your question might be answered
- **Ask specific questions**: Provide context and details
- **Be patient**: Maintainers are volunteers

### Helping Others

- **Answer questions**: Share your knowledge
- **Review pull requests**: Provide constructive feedback
- **Update documentation**: Fix issues you find
- **Welcome newcomers**: Be friendly and supportive

---

## Recognition

Contributors are recognized in several ways:

- **Contributors list**: Added to CONTRIBUTORS.md
- **Changelog mentions**: Significant contributions noted in releases
- **Commit history**: Your commits remain in the project history

---

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (see LICENSE file).

---

## Questions?

If you have questions about contributing:

1. Check this guide and other documentation
2. Search existing issues and discussions
3. Open a new discussion or issue
4. Tag maintainers if needed (but be patient!)

Thank you for contributing to Momoi! 🎨
