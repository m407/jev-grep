#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = []
# ///
"""
Agent Creator - Creates OpenCode agents from templates or specifications.

Usage:
    create_agent.py <name> --role <role> [options]
    create_agent.py <name> --template <template> [options]
    create_agent.py --list-templates
    create_agent.py --list-roles

Options:
    --role <role>           Agent role: reviewer, security, docs, tester, refactor, planner, orchestrator
    --template <template>   Use predefined template
    --mode <mode>           primary, subagent or all (default: subagent)
    --path <path>           Output directory (default: .opencode/agents)
    --model <model>         provider/model with optional #variant
    --temperature <temp>    Stored under request.body; currently inactive in the V2 runner
    --description <desc>    Custom description; prefer the formal input/output contract format
    --tools <actions>       Comma-separated permission actions to allow
    --no-tools <actions>    Comma-separated permission actions to deny (wins over allows)
    --dry-run               Print generated content without writing

Examples:
    create_agent.py code-reviewer --role reviewer
    create_agent.py security-audit --role security --mode subagent
    create_agent.py my-docs --template docs --path ~/.config/opencode/agents
    create_agent.py custom --role planner --description "Plans Android features"
"""

import argparse
import json
import sys
from pathlib import Path
from typing import Optional

CORE_ACTIONS = [
    "read",
    "edit",
    "shell",
    "grep",
    "glob",
    "skill",
    "webfetch",
    "websearch",
    "question",
    "subagent",
]

ACTION_ALIASES = {"bash": "shell", "task": "subagent", "write": "edit", "patch": "edit"}


def contract(
    summary: str, input_prompt: str, input_files_context: str, produces: str, output_message: str
) -> dict:
    """Build a standard description contract."""
    return {
        "summary": summary,
        "input_prompt": input_prompt,
        "input_files_context": input_files_context,
        "produces": produces,
        "output_message": output_message,
    }


# Role configurations
ROLES = {
    "reviewer": {
        "description": contract(
            summary="Reviews code for quality, security, and best practices. Use after code changes or before merging PRs.",
            input_prompt="must include review goal, target files or diff scope, and any specific risk areas.",
            input_files_context="repository source files, diffs, and relevant docs/specs.",
            produces="none",
            output_message="Responds with a review summary, categorized findings, recommendations, and a verdict.",
        ),
        "temperature": 0.2,
        "tools_enabled": ["read", "grep", "glob", "webfetch"],
        "tools_disabled": ["edit", "shell"],
        "prompt": """# Role
You are a senior code reviewer focusing on quality, security, and maintainability.

# Process
1. Identify changed/target files
2. Analyze code structure and patterns
3. Check for security vulnerabilities
4. Evaluate performance implications
5. Review code style and conventions
6. Provide actionable feedback

# Review Categories
- **Security**: Input validation, auth, data exposure
- **Performance**: Complexity, memory, caching
- **Maintainability**: Naming, structure, documentation
- **Style**: Conventions, formatting, consistency

# HITL Rules
- If critical security issue found -> flag immediately and STOP
- If architectural concerns -> ask for clarification before continuing

# Definition of Done
- [ ] All target files reviewed
- [ ] Issues categorized by severity (critical/major/minor)
- [ ] Recommendations provided with examples
- [ ] Summary with approve/request-changes verdict

# Output Format
## Summary
<overall assessment>

## Critical Issues
<list or "None found">

## Recommendations
<prioritized list>

## Verdict
<APPROVE | REQUEST_CHANGES | NEEDS_DISCUSSION>""",
    },
    "security": {
        "description": contract(
            summary="Performs security audits identifying vulnerabilities and compliance issues. Use for security reviews or before releases.",
            input_prompt="must include audit goal, scope, threat focus, and any compliance constraints.",
            input_files_context="repository source files, configs, dependency manifests, and security-relevant docs.",
            produces="none",
            output_message="Responds with an executive summary, prioritized findings, severity labels, and remediation guidance.",
        ),
        "temperature": 0.1,
        "tools_enabled": ["read", "grep", "glob", "webfetch"],
        "tools_disabled": ["edit", "shell"],
        "prompt": """# Role
You are a security expert focused on identifying vulnerabilities and risks.

# Process
1. Scan for sensitive data exposure (.env, credentials, API keys)
2. Check authentication and authorization patterns
3. Review input validation and sanitization
4. Analyze dependency vulnerabilities
5. Evaluate configuration security
6. Document findings with severity levels

# Focus Areas
- **Secrets**: Hardcoded credentials, API keys, tokens
- **Injection**: SQL, XSS, command injection vectors
- **Auth**: Broken authentication, session management
- **Data**: Sensitive data exposure, encryption
- **Config**: Security misconfigurations
- **Dependencies**: Known vulnerable packages

# HITL Rules
- If active credential exposure found -> STOP immediately and alert
- If critical vulnerability found -> document and request immediate review

# Definition of Done
- [ ] All source files scanned
- [ ] Secrets scan completed
- [ ] Vulnerabilities documented with severity
- [ ] Remediation steps provided
- [ ] Executive summary written

# Output Format
## Executive Summary
<high-level findings>

## Critical Findings
| Issue | Location | Severity | Remediation |
|-------|----------|----------|-------------|

## Recommendations
<prioritized security improvements>""",
    },
    "docs": {
        "description": contract(
            summary="Creates and updates project documentation. Use when docs need writing, updating, or improving.",
            input_prompt="must include documentation goal, audience, source material, and output scope.",
            input_files_context="repository files, existing docs, and related code or specs.",
            produces="documentation files under the requested path.",
            output_message="Responds with created or updated doc paths and a concise change summary.",
        ),
        "temperature": 0.5,
        "tools_enabled": ["read", "edit", "grep", "glob"],
        "tools_disabled": ["shell"],
        "prompt": """# Role
You are a technical writer creating clear, comprehensive documentation.

# Process
1. Understand the subject (code, API, feature)
2. Identify target audience
3. Structure content logically
4. Write clear explanations with examples
5. Add code samples where helpful
6. Review for completeness

# Documentation Types
- **README**: Project overview, setup, usage
- **API docs**: Endpoints, parameters, responses
- **Guides**: Step-by-step tutorials
- **Reference**: Technical specifications

# Style Guidelines
- Use clear, concise language
- Include practical examples
- Structure with headers and lists
- Add code blocks with syntax highlighting
- Link related documentation

# HITL Rules
- If technical details unclear -> ask for clarification
- If scope is large -> propose outline and wait for approval

# Definition of Done
- [ ] All sections complete
- [ ] Examples tested and working
- [ ] Links verified
- [ ] Spelling and grammar checked
- [ ] Format consistent with project style""",
    },
    "tester": {
        "description": contract(
            summary="Executes tests and reports results. Use to run unit tests, integration tests, or full test suites.",
            input_prompt="must include the test goal, target suite or command scope, and any environment constraints.",
            input_files_context="repository files, test configuration, and runtime output from executed test commands.",
            produces="none",
            output_message="Responds with pass/fail status, counts, failure details, and next-step recommendations.",
        ),
        "temperature": 0.1,
        "tools_enabled": ["read", "shell", "grep", "glob"],
        "tools_disabled": ["edit"],
        "permissions": [
            {"action": "shell", "resource": "*", "effect": "deny"},
            {"action": "shell", "resource": "npm test *", "effect": "allow"},
            {"action": "shell", "resource": "npm run test *", "effect": "allow"},
            {"action": "shell", "resource": "./gradlew test *", "effect": "allow"},
            {"action": "shell", "resource": "./gradlew :*:test *", "effect": "allow"},
            {"action": "shell", "resource": "pytest *", "effect": "allow"},
            {"action": "shell", "resource": "go test *", "effect": "allow"},
        ],
        "prompt": """# Role
You execute tests and provide clear reports on results.

# Process
1. Identify test command for project type
2. Execute tests
3. Parse results
4. Document failures with details
5. Provide summary

# Supported Test Frameworks
- **JavaScript/TypeScript**: npm test, jest, vitest
- **Android/Kotlin**: ./gradlew test
- **Python**: pytest
- **Go**: go test

# HITL Rules
- If test command unclear -> ask which tests to run
- If tests require setup -> ask for environment confirmation

# Definition of Done
- [ ] All requested tests executed
- [ ] Results parsed and formatted
- [ ] Failures documented with error messages
- [ ] Summary: passed/failed/skipped counts
- [ ] Recommendations for failures (if any)

# Output Format
## Test Results

**Status**: PASS | FAIL
**Passed**: X
**Failed**: Y
**Skipped**: Z

## Failures
<detailed failure info or "None">

## Recommendations
<suggestions for fixing failures>""",
    },
    "refactor": {
        "description": contract(
            summary="Assists with code refactoring while preserving behavior. Use for improving code structure, extracting methods, or modernizing patterns.",
            input_prompt="must include refactoring goal, target scope, constraints, and any required verification.",
            input_files_context="repository source files, tests, and relevant design context.",
            produces="updated source files in the requested scope.",
            output_message="Responds with changed file paths, refactoring rationale, and validation status.",
        ),
        "temperature": 0.3,
        "tools_enabled": ["read", "edit", "shell", "grep", "glob"],
        "tools_disabled": [],
        "permissions": [
            {"action": "shell", "resource": "*", "effect": "ask"},
            {"action": "shell", "resource": "git status *", "effect": "allow"},
            {"action": "shell", "resource": "git diff *", "effect": "allow"},
        ],
        "prompt": """# Role
You are a refactoring expert improving code structure while preserving behavior.

# Process
1. Understand current code structure
2. Identify refactoring opportunities
3. Plan changes (small, incremental steps)
4. Apply refactoring
5. Verify behavior preserved (tests pass)
6. Document changes

# Refactoring Types
- **Extract**: Method, class, interface
- **Rename**: Variables, functions, classes
- **Move**: Relocate code to better location
- **Simplify**: Reduce complexity, remove duplication
- **Modernize**: Update to newer patterns/APIs

# HITL Rules
- If refactoring affects public API -> STOP and request approval
- If changes exceed original scope -> ask before proceeding
- If tests fail after refactoring -> report and wait for guidance

# Definition of Done
- [ ] Refactoring applied
- [ ] Tests still pass
- [ ] No behavior changes (unless intended)
- [ ] Code cleaner and more maintainable
- [ ] Changes documented

# Safety Rules
- Make small, incremental changes
- Run tests after each change
- Preserve all existing functionality
- Document any intentional behavior changes""",
    },
    "planner": {
        "description": contract(
            summary="Analyzes code and creates plans without making changes. Use for architecture review, planning, or understanding codebases.",
            input_prompt="must include the planning goal, scope, constraints, and desired output depth.",
            input_files_context="repository files, relevant docs, and any referenced design materials.",
            produces="none",
            output_message="Responds with analysis, recommendations, and an implementation plan when applicable.",
        ),
        "temperature": 0.3,
        "tools_enabled": ["read", "grep", "glob", "webfetch", "subagent"],
        "tools_disabled": ["edit", "shell"],
        "prompt": """# Role
You analyze code and create plans without making any changes.

# Capabilities
- Explore and understand codebase structure
- Analyze architecture and patterns
- Create implementation plans
- Review and suggest improvements
- Research solutions

# Process
1. Understand the request
2. Explore relevant code
3. Analyze patterns and structure
4. Create detailed plan or analysis
5. Present findings with recommendations

# HITL Rules
- If analysis scope is unclear -> ask for clarification
- If multiple approaches possible -> present options and wait for selection

# Definition of Done
- [ ] Relevant files and docs analyzed
- [ ] Key constraints and risks identified
- [ ] Recommendations provided
- [ ] Implementation plan written when applicable

# Output Format
## Analysis
<findings>

## Recommendations
<suggestions>

## Implementation Plan (if applicable)
1. Step 1
2. Step 2
...""",
    },
    "orchestrator": {
        "description": contract(
            summary="Coordinates complex multi-step workflows by delegating to specialized subagents. Use for pipelines requiring multiple specialized steps.",
            input_prompt="must include the end goal, phase boundaries, constraints, and expected deliverables.",
            input_files_context="task requirements, phase outputs, and referenced repository files or docs.",
            produces="workflow artifacts only when explicitly requested; otherwise none.",
            output_message="Responds with phase status, validation checkpoints, and final delivery summary.",
        ),
        "temperature": 0.3,
        "tools_enabled": ["read", "grep", "glob", "subagent", "edit"],
        "tools_disabled": ["shell"],
        "prompt": """# Role
You coordinate complex workflows by delegating to specialized subagents.

# Process
1. Analyze request -> break into phases
2. Identify required subagents for each phase
3. Execute phases in order
4. Validate outputs between phases
5. Report final status

# Coordination Rules
- Execute phases sequentially (unless explicitly parallelizable)
- Validate each phase output before proceeding
- Document progress in manifest/log
- Handle failures gracefully

# HITL Rules
- If phase fails -> report and ask how to proceed
- If scope changes mid-workflow -> confirm new direction
- Before final delivery -> summarize and request approval

# Definition of Done
- [ ] All phases completed successfully
- [ ] Outputs validated
- [ ] Summary provided
- [ ] Ready for delivery""",
    },
}


def normalize_tools_csv(value: Optional[str]) -> list[str] | None:
    """Parse comma-separated tool lists."""
    if value is None:
        return None

    tools = [tool.strip() for tool in value.split(",") if tool.strip()]
    return tools or None


def ensure_description_contract(description: str, role_contract: dict) -> str:
    """Normalize description text to the formal contract layout."""
    description = description.strip()
    if "Meta:" in description:
        return description

    summary = description or role_contract["summary"]
    lines = [
        summary,
        "Meta:",
        f"- Input prompt: {role_contract['input_prompt']}",
        f"- Consumes: {role_contract['input_files_context']}",
        f"- Produces: {role_contract['produces']}",
        f"- Output message: {role_contract['output_message']}",
    ]
    return "\n".join(lines)


def generate_description_yaml(description: str) -> str:
    """Render description using the documented YAML block format."""
    lines = ["description: >"]
    for line in description.splitlines():
        lines.append(f"  {line}" if line else "  ")
    return "\n".join(lines)


def normalize_action(action: str) -> str:
    """Map supported legacy CLI aliases to native V2 permission actions."""
    if action in {"list", "todowrite", "todoread", "lsp", "doom_loop"}:
        raise ValueError(f"Unsupported V2 Core action: {action}")
    return ACTION_ALIASES.get(action, action)


def generate_permissions_yaml(permissions: list[dict[str, str]]) -> str:
    """Render ordered V2 rules, quoting values safely for YAML."""
    lines = ["permissions:"]
    for rule in permissions:
        lines.extend(
            [
                f"  - action: {json.dumps(rule['action'])}",
                f"    resource: {json.dumps(rule['resource'])}",
                f"    effect: {rule['effect']}",
            ]
        )
    return "\n".join(lines)


def generate_agent(
    name: str,
    role: str,
    mode: str = "subagent",
    model: Optional[str] = None,
    temperature: Optional[float] = None,
    description: Optional[str] = None,
    extra_tools_enabled: Optional[list] = None,
    extra_tools_disabled: Optional[list] = None,
) -> str:
    """Generate agent markdown content."""

    if role not in ROLES:
        raise ValueError(f"Unknown role: {role}. Available: {', '.join(ROLES.keys())}")
    if mode not in {"primary", "subagent", "all"}:
        raise ValueError(f"Unknown mode: {mode}")
    if temperature is not None and not 0.0 <= temperature <= 1.0:
        raise ValueError("Temperature must be between 0.0 and 1.0")

    config = ROLES[role]
    desc = ensure_description_contract(
        description or config["description"]["summary"], config["description"]
    )

    # Build frontmatter
    frontmatter_lines = ["---"]

    # Description
    frontmatter_lines.append(generate_description_yaml(desc))

    # Mode
    frontmatter_lines.append(f"mode: {mode}")

    # Model (optional)
    if model:
        frontmatter_lines.append(f"model: {json.dumps(model)}")

    # Temperature
    temp = temperature if temperature is not None else config.get("temperature")
    if temp is not None:
        frontmatter_lines.extend(["request:", "  body:", f"    temperature: {temp}"])

    # Known core actions first, then role exceptions and explicit CLI overrides.
    states = dict.fromkeys(CORE_ACTIONS, "deny")
    for action in config.get("tools_enabled", []):
        states[normalize_action(action)] = "allow"
    for action in config.get("tools_disabled", []):
        states[normalize_action(action)] = "deny"
    permissions = [
        {"action": action, "resource": "*", "effect": effect}
        for action, effect in states.items()
    ]
    permissions.extend(config.get("permissions", []))
    for actions, effect in (
        (extra_tools_enabled or [], "allow"),
        (extra_tools_disabled or [], "deny"),
    ):
        for action in actions:
            permissions.append(
                {"action": normalize_action(action), "resource": "*", "effect": effect}
            )
    frontmatter_lines.append(generate_permissions_yaml(permissions))

    frontmatter_lines.append("---")

    # Combine frontmatter and prompt
    frontmatter = "\n".join(frontmatter_lines)
    prompt = config["prompt"]

    return f"{frontmatter}\n\n{prompt}\n"


def list_templates():
    """List available templates/roles."""
    print("Available roles/templates:\n")
    for name, config in ROLES.items():
        summary = config["description"]["summary"]
        desc = summary[:60] + "..." if len(summary) > 60 else summary
        print(f"  {name:12} - {desc}")
    print("\nUsage: create_agent.py <name> --role <role>")


def main():
    parser = argparse.ArgumentParser(
        description="Create OpenCode agents from templates",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  create_agent.py code-reviewer --role reviewer
  create_agent.py security-audit --role security --mode subagent
  create_agent.py my-planner --role planner --model anthropic/claude-sonnet-4-20250514
  create_agent.py custom --role docs --description "Writes API documentation"
        """,
    )

    parser.add_argument("name", nargs="?", help="Agent name (becomes filename)")
    parser.add_argument(
        "--role", "-r", choices=list(ROLES.keys()), help="Agent role/template"
    )
    parser.add_argument(
        "--template", "-t", choices=list(ROLES.keys()), help="Alias for --role"
    )
    parser.add_argument(
        "--mode",
        "-m",
        choices=["primary", "subagent", "all"],
        default="subagent",
        help="Agent mode",
    )
    parser.add_argument(
        "--path", "-p", default=".opencode/agents", help="Output directory"
    )
    parser.add_argument(
        "--model", help="provider/model with optional #variant"
    )
    parser.add_argument(
        "--temperature",
        type=float,
        help="Stored request.body temperature (0.0-1.0); currently inactive in the V2 runner",
    )
    parser.add_argument(
        "--description",
        "-d",
        help="Custom description; prefer the formal input/output contract format",
    )
    parser.add_argument("--tools", help="Comma-separated permission actions to allow")
    parser.add_argument("--no-tools", help="Comma-separated actions to deny (wins over allows)")
    parser.add_argument("--dry-run", action="store_true", help="Print without writing")
    parser.add_argument(
        "--list-templates",
        "--list-roles",
        action="store_true",
        help="List available templates/roles",
    )

    args = parser.parse_args()

    # Handle list templates
    if args.list_templates:
        list_templates()
        return 0

    # Validate required args
    if not args.name:
        parser.print_help()
        return 1

    role = args.role or args.template
    if not role:
        print("Error: --role or --template is required")
        print(f"Available roles: {', '.join(ROLES.keys())}")
        return 1

    # Parse tools
    extra_enabled = normalize_tools_csv(args.tools)
    extra_disabled = normalize_tools_csv(args.no_tools)

    if args.temperature is not None and not 0.0 <= args.temperature <= 1.0:
        print("Error: --temperature must be between 0.0 and 1.0")
        return 1

    # Generate agent content
    try:
        content = generate_agent(
            name=args.name,
            role=role,
            mode=args.mode,
            model=args.model,
            temperature=args.temperature,
            description=args.description,
            extra_tools_enabled=extra_enabled,
            extra_tools_disabled=extra_disabled,
        )
    except ValueError as e:
        print(f"Error: {e}")
        return 1

    # Output
    if args.dry_run:
        print(f"# Would write to: {args.path}/{args.name}.md\n")
        print(content)
        return 0

    # Write file
    output_dir = Path(args.path).expanduser()
    output_dir.mkdir(parents=True, exist_ok=True)

    output_file = output_dir / f"{args.name}.md"

    if output_file.exists():
        print(f"Error: File already exists: {output_file}")
        print("Use --dry-run to preview or delete existing file first")
        return 1

    output_file.write_text(content)
    print(f"Created agent: {output_file}")
    print(f"\nAgent ID: {args.name} (mode: {args.mode})")
    if args.mode in {"subagent", "all"}:
        print(f"Ask your primary agent to use the {args.name} subagent.")

    return 0


if __name__ == "__main__":
    sys.exit(main())
