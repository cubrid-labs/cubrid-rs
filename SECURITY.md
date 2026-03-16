# Security Policy

## Supported Versions

| Version | Status |
|---------|--------|
| 0.1.x   | ✅ Supported |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue in cubrid-rs, please report it responsibly by emailing:

**Email:** paikend@gmail.com

**Do not** open a public GitHub issue for security vulnerabilities. Responsible disclosure allows us to address the issue before public disclosure.

### Response Timeline

- **48 hours:** Initial acknowledgment of your report
- **7 days:** Security assessment and initial response with remediation plan
- **Ongoing:** Regular updates on progress until resolution

## What Qualifies as a Security Issue

A security issue is any vulnerability that could:

- Allow unauthorized access to data
- Permit SQL injection or other code execution attacks
- Expose sensitive information (credentials, tokens, private data)
- Compromise confidentiality, integrity, or availability of the system

Examples include:

- SQL injection vulnerabilities in query construction
- Authentication/authorization flaws
- Insecure credential handling
- Protocol parsing flaws that lead to memory or availability issues

## Security Best Practices for Users

When using cubrid-rs, follow these security best practices:

- Always use parameterized query APIs where available
- Keep cubrid-rs updated to the latest version
- Use secure connection and network policies for database traffic
- Follow least privilege for database credentials
- Never hardcode credentials in application source
- Use environment variables or secret management systems

## Disclosure Policy

Once a security vulnerability is fixed:

1. A security patch will be released
2. The vulnerability will be disclosed in release notes
3. Credit will be given to the reporter (if requested)

We appreciate responsible disclosure and help in keeping cubrid-rs secure.
