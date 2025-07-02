# Gil-Flow Contribution Guidelines

This document outlines the guiding principles for contributing to the Gil-Flow project. Adhere to these guidelines to ensure consistency, quality, and a shared understanding of the project's architecture and goals.

## Documentation

**Structure and Reference:**
*   **/README.md**: Project overview, core philosophy, and a concise quick-start guide.
*   **/docs/YAML_SPEC.md**: The standard for Gil-Flow's YAML syntax, defining language-neutral workflow structures.
*   **/docs/NODE_SPEC.md**: Specifications for standard node types and their interfaces, common to all implementations.
*   **/docs/ARCHITECTURE.md**: The language-neutral architecture guide, ensuring compatibility and extensibility across different implementations.
*   **/docs/DEV.md**: A guide for developing the `gil-py` implementation, detailing its Python-specific internal structure.
*   **/TASKS.md**: Tracks the status and plans for development tasks. Keep this updated when you start or complete a task.

**Principles:**
*   **Clarity and Focus**: Each document must have a single, clear purpose with minimal content overlap.
*   **README is for Onboarding**: The main `README.md` should only contain a high-level overview and quick-start instructions. Link to other documents for details.
*   **Conceptual Examples**: Prefer pseudocode for explaining concepts to keep the focus on the idea, not the implementation.
*   **Inter-document Linking**: Use cross-references to connect related documents.
*   **Conciseness**: Write clearly and concisely, avoiding verbosity and unnecessary complexity.
*   **English Only**: All documentation, code, and comments must be written in English for consistency and accessibility.

## Code Quality

*   **Readability First**: Write code that is easy to read and understand.
*   **Meaningful Naming**: Use clear and descriptive names for all variables, functions, and classes.
*   **Self-Documenting Code**: Strive to write code that is clear enough to make comments unnecessary. Add comments only to explain the *why*, not the *what*.
*   **Modularity**: Avoid code duplication by creating reusable modules.
*   **Simplicity**: Decompose complex logic into smaller, manageable units.
*   **Single Responsibility Principle (SRP)**: Ensure every module or class has one, and only one, reason to change.
*   **SOLID Foundation**: Base your designs on SOLID principles.
*   **Consistency**: Maintain a consistent coding style across the entire codebase.
*   **Minimize Comments**: Write code so clear that comments become unnecessary. When comments are needed, explain the reasoning, not the mechanics.

## Problem-Solving

*   **Root Cause Analysis**: Avoid temporary fixes. Always address the underlying cause of a problem with a structural and systematic approach.
*   **Simplicity and Clarity**: Aim to solve even complex problems using simple and clear methods.
*   **Direct Solutions**: Identify the core of the problem and implement a direct and effective solution.
*   **Pragmatism**: Prioritize practical, working solutions that improve the existing structure.
*   **Structural Approach**: Prefer systematic solutions over quick patches or workarounds.
*   **Balance Improvement**: Consider both existing structure enhancement and practical solutions in a balanced manner.

## Development Philosophy

*   **Avoid Over-Engineering**: Develop solutions that meet current requirements without unnecessary complexity.
*   **Justified Design Patterns**: Introduce design patterns only when there is a clear and present need with demonstrable code improvement benefits.
*   **Sufficient Design**: Maintain a design that is necessary and sufficient for the current context.
*   **Focus on the Present**: Solve today's problems instead of building for uncertain future needs.
*   **YAGNI (You Aren't Gonna Need It)**: Do not implement features that are not required right now.
*   **Appropriate Development**: Build according to current requirements, avoiding both under-engineering and over-engineering.

## Refactoring

*   **Be Bold**: Do not hesitate to refactor legacy code if a better solution exists.
*   **Prioritize Structural Improvement**: If existing code is an obstacle to a robust solution, improve the structure first.
*   **Embrace Change**: If a complete redesign is more efficient than incremental changes, opt for the more significant change.
*   **Simplify Legacy Systems**: If a legacy system's complexity is hurting productivity, redesign it with a simpler and clearer structure.
*   **Manage Technical Debt**: When maintenance costs due to technical debt exceed the cost of a rewrite, a full refactoring is warranted.
*   **Comprehensive Restructuring**: When comprehensive restructuring is more efficient than gradual improvements, make bold changes.
*   **Execute Full Refactoring**: When technical debt costs exceed redevelopment costs, execute complete refactoring.

## Testing Philosophy

*   **Quality Assurance Tool**: Tests are tools for quality assurance, not absolute standards or goals in themselves.
*   **Failure Analysis**: When tests fail, analyze the cause to determine whether to fix the source code or update the tests.
*   **Intentional Improvements**: When intentional improvements cause test failures, update the tests to reflect the new expectations.
*   **Appropriate Coverage**: Maintain appropriate test levels without hindering productivity through excessive testing.
*   **Meaningful Tests Only**: Write tests that provide real value and avoid pursuing coverage metrics for their own sake.
*   **Avoid Formalistic Coverage**: Focus on meaningful test scenarios rather than achieving arbitrary coverage percentages.

## Language and Communication

*   **English Standard**: All code, documentation, comments, commit messages, and project communication must be in English.
*   **Global Accessibility**: Ensure the project remains accessible to the international development community.
*   **Consistent Communication**: Maintain consistent language standards across all project artifacts.