#!/usr/bin / env tsx
/* oxlint-disable */
// Flags exported values, members, and parameters in src/ referenced only by tests.
// Distinguishes LEAKED_PRIVATE_IMPL, TEST_ONLY_ORPHAN, and TEST_ONLY_PARAM.

import path from 'node:path';
import ts from 'typescript';

const cwd = process.cwd();
const jsonMode = process.argv.includes('--json');

const configPath = ts.findConfigFile(cwd, ts.sys.fileExists, 'tsconfig.json');
if (!configPath) {
  console.error('Could not find tsconfig.json');
  process.exit(1);
}

const { config } = ts.readConfigFile(configPath, ts.sys.readFile);
const parsed = ts.parseJsonConfigFileContent(config, ts.sys, path.dirname(configPath));
const program = ts.createProgram(parsed.fileNames, parsed.options);
const checker = program.getTypeChecker();

const isTestFile = (f: string) =>
  f.includes('/tests/') || /\.(test|spec)\.[cm]?[jt]sx?$/.test(f);

const NEXT_ENTRY_FILE_RE =
  /\/src\/app\/(?:.*\/)?(page|layout|route|loading|error|global-error|not-found|template|default|opengraph-image|twitter-image|icon|apple-icon|manifest|robots|sitemap)\.tsx?$/;

const NEXT_CONVENTION_EXPORTS = new Set([
  'default',
  'GET',
  'POST',
  'PUT',
  'PATCH',
  'DELETE',
  'HEAD',
  'OPTIONS',
  'metadata',
  'generateMetadata',
  'generateStaticParams',
  'viewport',
  'generateViewport',
  'dynamic',
  'dynamicParams',
  'revalidate',
  'fetchCache',
  'runtime',
  'preferredRegion',
  'maxDuration',
  'size',
  'contentType',
  'alt',
  'Image',
  'middleware',
  'config',
]);

const hasExportModifier = (node: ts.Node) =>
  ts.canHaveModifiers(node) &&
  Boolean(ts.getModifiers(node)?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword));

const hasDefaultModifier = (node: ts.Node) =>
  ts.canHaveModifiers(node) &&
  Boolean(ts.getModifiers(node)?.some((m) => m.kind === ts.SyntaxKind.DefaultKeyword));

interface ParamCandidate {
  index: number;
  name: string;
  line: number;
}

type CandidateKind = 'function' | 'const' | 'let' | 'var' | 'class' | 'enum' | 'property';

interface Candidate {
  symbol: ts.Symbol;
  nameNode: ts.Identifier;
  declNode: ts.Node;
  sourceFile: ts.SourceFile;
  file: string;
  line: number;
  kind: CandidateKind;
  name: string;
  fromInlineTypeLiteral: boolean;
  sameFileRefs: number;
  prodExternalRefs: number;
  wholeValueProdRefs: number;
  testRefs: number;
  testFiles: Set<string>;
  params: ParamCandidate[];
  prodCallCount: number;
  prodMaxArgs: number;
  testCallCount: number;
  testMaxArgs: number;
  nonCallProdRefs: number;
}

const candidatesBySymbol = new Map<ts.Symbol, Candidate>();
const candidateNames = new Set<string>();
const propertyCandidatesByObjSymbol = new Map<ts.Symbol, Map<string, Candidate>>();

function resolveRootSymbol(sym: ts.Symbol | undefined): ts.Symbol | undefined {
  if (!sym) return undefined;
  if (sym.flags & ts.SymbolFlags.Alias) {
    try {
      return checker.getAliasedSymbol(sym);
    } catch {
      return sym;
    }
  }
  return sym;
}

function getReferencedSymbol(node: ts.Identifier): ts.Symbol | undefined {
  if (ts.isShorthandPropertyAssignment(node.parent) && node.parent.name === node) {
    return resolveRootSymbol(checker.getShorthandAssignmentValueSymbol(node.parent));
  }
  return resolveRootSymbol(checker.getSymbolAtLocation(node));
}

function isInTypePosition(node: ts.Node): boolean {
  let cur: ts.Node | undefined = node.parent;
  while (cur) {
    if (ts.isTypeNode(cur) || ts.isTypeAliasDeclaration(cur) || ts.isInterfaceDeclaration(cur)) {
      return true;
    }
    cur = cur.parent;
  }
  return false;
}

function collectBindingIdentifiers(
  name: ts.BindingName,
  out: { id: ts.Identifier; declNode: ts.Node }[],
  fallbackDecl: ts.Node,
): void {
  if (ts.isIdentifier(name)) {
    out.push({ id: name, declNode: fallbackDecl });
  } else if (ts.isObjectBindingPattern(name)) {
    for (const el of name.elements) {
      collectBindingIdentifiers(el.name, out, el);
    }
  } else if (ts.isArrayBindingPattern(name)) {
    for (const el of name.elements) {
      if (ts.isBindingElement(el)) {
        collectBindingIdentifiers(el.name, out, el);
      }
    }
  }
}

function unwrapExpression(expr: ts.Expression): ts.Expression {
  let cur = expr;
  while (
    ts.isAsExpression(cur) ||
    ts.isSatisfiesExpression(cur) ||
    ts.isParenthesizedExpression(cur)
  ) {
    cur = cur.expression;
  }
  return cur;
}

function extractFunctionNode(declNode: ts.Node): ts.SignatureDeclarationBase | undefined {
  if (ts.isFunctionDeclaration(declNode) || ts.isMethodDeclaration(declNode)) return declNode;
  if (
    (ts.isVariableDeclaration(declNode) || ts.isPropertyAssignment(declNode)) &&
    declNode.initializer
  ) {
    const init = unwrapExpression(declNode.initializer);
    if (ts.isArrowFunction(init) || ts.isFunctionExpression(init)) return init;
  }
  return undefined;
}

function extractParams(declNode: ts.Node, sf: ts.SourceFile): ParamCandidate[] {
  const fn = extractFunctionNode(declNode);
  if (!fn) return [];
  return fn.parameters.map((p, index) => {
    const { line } = sf.getLineAndCharacterOfPosition(p.getStart(sf));
    return { index, name: p.name.getText(sf), line: line + 1 };
  });
}

// Pass 1: Collect exported declarations and object members in `src/`
for (const sf of program.getSourceFiles()) {
  if (sf.isDeclarationFile || sf.fileName.includes('node_modules')) continue;
  if (isTestFile(sf.fileName)) continue;
  if (!sf.fileName.includes('/src/')) continue;

  const isNextEntry =
    NEXT_ENTRY_FILE_RE.test(sf.fileName) ||
    /\/src\/(middleware|instrumentation)\.ts$/.test(sf.fileName);

  const registerCandidate = (
    nameNode: ts.Identifier,
    declNode: ts.Node,
    kind: CandidateKind,
  ): Candidate | undefined => {
    const name = nameNode.text;
    if (isNextEntry && NEXT_CONVENTION_EXPORTS.has(name)) return undefined;

    const sym = resolveRootSymbol(checker.getSymbolAtLocation(nameNode));
    if (!sym) return undefined;
    const existing = candidatesBySymbol.get(sym);
    if (existing) return existing;

    const { line } = sf.getLineAndCharacterOfPosition(nameNode.getStart(sf));
    const candidate: Candidate = {
      symbol: sym,
      nameNode,
      declNode,
      sourceFile: sf,
      file: path.relative(cwd, sf.fileName),
      line: line + 1,
      kind,
      name,
      fromInlineTypeLiteral: false,
      sameFileRefs: 0,
      prodExternalRefs: 0,
      wholeValueProdRefs: 0,
      testRefs: 0,
      testFiles: new Set<string>(),
      params: extractParams(declNode, sf),
      prodCallCount: 0,
      prodMaxArgs: 0,
      testCallCount: 0,
      testMaxArgs: 0,
      nonCallProdRefs: 0,
    };
    candidatesBySymbol.set(sym, candidate);
    candidateNames.add(name);
    return candidate;
  };

  // Register members of exported objects (both inline type literals and object literal initializers)
  const registerObjectMembers = (parentCandidate: Candidate, decl: ts.VariableDeclaration) => {
    const propMap =
      propertyCandidatesByObjSymbol.get(parentCandidate.symbol) ?? new Map<string, Candidate>();

    const addPropCandidate = (
      propIdent: ts.Identifier,
      memberNode: ts.Node,
      fromInlineTypeLiteral: boolean,
    ) => {
      const propName = propIdent.text;
      const existing = propMap.get(propName);
      if (existing) {
        if (fromInlineTypeLiteral) existing.fromInlineTypeLiteral = true;
        return;
      }
      const { line } = sf.getLineAndCharacterOfPosition(propIdent.getStart(sf));
      const propCandidate: Candidate = {
        symbol: parentCandidate.symbol,
        nameNode: propIdent,
        declNode: memberNode,
        sourceFile: sf,
        file: path.relative(cwd, sf.fileName),
        line: line + 1,
        kind: 'property',
        name: `${parentCandidate.name}.${propName}`,
        fromInlineTypeLiteral,
        sameFileRefs: 0,
        prodExternalRefs: 0,
        wholeValueProdRefs: 0,
        testRefs: 0,
        testFiles: new Set<string>(),
        params: extractParams(memberNode, sf),
        prodCallCount: 0,
        prodMaxArgs: 0,
        testCallCount: 0,
        testMaxArgs: 0,
        nonCallProdRefs: 0,
      };
      propMap.set(propName, propCandidate);
      candidateNames.add(propName);
    };

    if (decl.initializer) {
      const init = unwrapExpression(decl.initializer);
      if (ts.isObjectLiteralExpression(init)) {
        for (const prop of init.properties) {
          if (
            (ts.isPropertyAssignment(prop) || ts.isMethodDeclaration(prop)) &&
            prop.name &&
            ts.isIdentifier(prop.name)
          ) {
            addPropCandidate(prop.name, prop, false);
          } else if (ts.isShorthandPropertyAssignment(prop)) {
            addPropCandidate(prop.name, prop, false);
          }
        }
      }
    }

    if (decl.type) {
      const typeLiterals: ts.TypeLiteralNode[] = [];
      if (ts.isIntersectionTypeNode(decl.type)) {
        for (const t of decl.type.types) {
          if (ts.isTypeLiteralNode(t)) typeLiterals.push(t);
        }
      } else if (ts.isTypeLiteralNode(decl.type)) {
        typeLiterals.push(decl.type);
      }
      for (const lit of typeLiterals) {
        for (const member of lit.members) {
          if (
            (ts.isPropertySignature(member) || ts.isMethodSignature(member)) &&
            member.name &&
            ts.isIdentifier(member.name)
          ) {
            addPropCandidate(member.name, member, true);
          }
        }
      }
    }

    if (propMap.size > 0) {
      propertyCandidatesByObjSymbol.set(parentCandidate.symbol, propMap);
    }
  };

  sf.forEachChild((node) => {
    if (ts.isFunctionDeclaration(node) && hasExportModifier(node) && node.name) {
      if (isNextEntry && hasDefaultModifier(node)) return;
      registerCandidate(node.name, node, 'function');
    } else if (ts.isClassDeclaration(node) && hasExportModifier(node) && node.name) {
      if (isNextEntry && hasDefaultModifier(node)) return;
      registerCandidate(node.name, node, 'class');
    } else if (ts.isEnumDeclaration(node) && hasExportModifier(node) && node.name) {
      registerCandidate(node.name, node, 'enum');
    } else if (ts.isVariableStatement(node) && hasExportModifier(node)) {
      const kw =
        node.declarationList.flags & ts.NodeFlags.Const
          ? 'const'
          : node.declarationList.flags & ts.NodeFlags.Let
            ? 'let'
            : 'var';
      for (const decl of node.declarationList.declarations) {
        const bindings: { id: ts.Identifier; declNode: ts.Node }[] = [];
        collectBindingIdentifiers(decl.name, bindings, decl);
        for (const b of bindings) {
          const c = registerCandidate(b.id, b.declNode, kw);
          if (c && ts.isIdentifier(decl.name)) {
            registerObjectMembers(c, decl);
          }
        }
      }
    } else if (
      ts.isExportDeclaration(node) &&
      !node.isTypeOnly &&
      !node.moduleSpecifier &&
      node.exportClause &&
      ts.isNamedExports(node.exportClause)
    ) {
      // Handle `export { foo, bar as baz }` without `from`
      for (const spec of node.exportClause.elements) {
        if (spec.isTypeOnly) continue;
        candidateNames.add(spec.name.text);
        const targetIdent = spec.propertyName ?? spec.name;
        const sym = resolveRootSymbol(checker.getSymbolAtLocation(targetIdent));
        if (!sym || candidatesBySymbol.has(sym)) continue;
        const decl = sym.valueDeclaration ?? sym.declarations?.[0];
        if (!decl || decl.getSourceFile().fileName !== sf.fileName) continue;
        if (ts.isFunctionDeclaration(decl) && decl.name) {
          registerCandidate(decl.name, decl, 'function');
        } else if (ts.isClassDeclaration(decl) && decl.name) {
          registerCandidate(decl.name, decl, 'class');
        } else if (ts.isEnumDeclaration(decl) && decl.name) {
          registerCandidate(decl.name, decl, 'enum');
        } else if (ts.isVariableDeclaration(decl) && ts.isIdentifier(decl.name)) {
          const c = registerCandidate(decl.name, decl, 'const');
          if (c) registerObjectMembers(c, decl);
        } else if (ts.isBindingElement(decl) && ts.isIdentifier(decl.name)) {
          registerCandidate(decl.name, decl, 'const');
        }
      }
    }
  });
}

// Pre-pass: Collect import aliases, default imports, re-exports, and destructurings
for (const sf of program.getSourceFiles()) {
  if (sf.isDeclarationFile || sf.fileName.includes('node_modules')) continue;

  const visitAliases = (node: ts.Node) => {
    if (ts.isImportDeclaration(node) && node.importClause) {
      if (node.importClause.name) {
        const sym = resolveRootSymbol(checker.getSymbolAtLocation(node.importClause.name));
        if (sym && candidatesBySymbol.has(sym)) {
          candidateNames.add(node.importClause.name.text);
        }
      }
      if (
        node.importClause.namedBindings &&
        ts.isNamedImports(node.importClause.namedBindings)
      ) {
        for (const el of node.importClause.namedBindings.elements) {
          const sym = resolveRootSymbol(checker.getSymbolAtLocation(el.name));
          if (sym && candidatesBySymbol.has(sym)) {
            candidateNames.add(el.name.text);
          } else if (el.propertyName && candidateNames.has(el.propertyName.text)) {
            candidateNames.add(el.name.text);
          }
        }
      }
    } else if (
      ts.isExportDeclaration(node) &&
      !node.isTypeOnly &&
      node.exportClause &&
      ts.isNamedExports(node.exportClause)
    ) {
      for (const spec of node.exportClause.elements) {
        if (spec.isTypeOnly) continue;
        const sym = resolveRootSymbol(checker.getSymbolAtLocation(spec.name));
        if (sym && candidatesBySymbol.has(sym)) {
          candidateNames.add(spec.name.text);
        } else if (spec.propertyName && candidateNames.has(spec.propertyName.text)) {
          candidateNames.add(spec.name.text);
        }
      }
    } else if (
      ts.isVariableDeclaration(node) &&
      ts.isObjectBindingPattern(node.name) &&
      node.initializer
    ) {
      const rhsType = checker.getTypeAtLocation(node.initializer);
      for (const el of node.name.elements) {
        if (!ts.isIdentifier(el.name)) continue;
        const propIdent = el.propertyName ?? el.name;
        if (!ts.isIdentifier(propIdent)) continue;
        const propSym = resolveRootSymbol(checker.getPropertyOfType(rhsType, propIdent.text));
        if (propSym && candidatesBySymbol.has(propSym)) {
          const targetCandidate = candidatesBySymbol.get(propSym)!;
          const localSym = checker.getSymbolAtLocation(el.name);
          if (localSym && !candidatesBySymbol.has(localSym)) {
            candidatesBySymbol.set(localSym, targetCandidate);
          }
          candidateNames.add(el.name.text);
        }
      }
    }
    ts.forEachChild(node, visitAliases);
  };
  visitAliases(sf);
}

function recordCallArgs(candidate: Candidate, node: ts.Identifier, isTest: boolean): boolean {
  if (ts.isCallExpression(node.parent) && node.parent.expression === node) {
    if (isTest) {
      candidate.testCallCount++;
      candidate.testMaxArgs = Math.max(candidate.testMaxArgs, node.parent.arguments.length);
    } else {
      candidate.prodCallCount++;
      candidate.prodMaxArgs = Math.max(candidate.prodMaxArgs, node.parent.arguments.length);
    }
    return true;
  }
  if (
    ts.isPropertyAccessExpression(node.parent) &&
    node.parent.name === node &&
    ts.isCallExpression(node.parent.parent) &&
    node.parent.parent.expression === node.parent
  ) {
    if (isTest) {
      candidate.testCallCount++;
      candidate.testMaxArgs = Math.max(candidate.testMaxArgs, node.parent.parent.arguments.length);
    } else {
      candidate.prodCallCount++;
      candidate.prodMaxArgs = Math.max(candidate.prodMaxArgs, node.parent.parent.arguments.length);
    }
    return true;
  }
  return false;
}

// Pass 2: Walk all files once and attribute identifier references to candidates
for (const sf of program.getSourceFiles()) {
  if (sf.isDeclarationFile || sf.fileName.includes('node_modules')) continue;

  const inTest = isTestFile(sf.fileName);
  const relFile = path.relative(cwd, sf.fileName);
  const isNextEntry =
    NEXT_ENTRY_FILE_RE.test(sf.fileName) ||
    /\/src\/(middleware|instrumentation)\.ts$/.test(sf.fileName);

  const attributePropertyCandidate = (
    propCandidate: Candidate,
    accessNode: ts.Identifier,
  ) => {
    if (accessNode === propCandidate.nameNode) return;
    if (isInTypePosition(accessNode)) return;
    if (sf.fileName === propCandidate.sourceFile.fileName) {
      const withinOwnDecl =
        accessNode.getStart(sf) >= propCandidate.declNode.getStart(sf) &&
        accessNode.getEnd() <= propCandidate.declNode.getEnd();
      if (!withinOwnDecl) {
        propCandidate.sameFileRefs++;
        recordCallArgs(propCandidate, accessNode, false);
      }
    } else if (inTest) {
      propCandidate.testRefs++;
      propCandidate.testFiles.add(relFile);
      recordCallArgs(propCandidate, accessNode, true);
    } else {
      propCandidate.prodExternalRefs++;
      if (!recordCallArgs(propCandidate, accessNode, false)) {
        propCandidate.nonCallProdRefs++;
      }
    }
  };

  const visit = (node: ts.Node) => {
    if (ts.isIdentifier(node) && candidateNames.has(node.text)) {
      // Check property accesses (e.g. `userService.listUsers(...)` or `ns.foo`)
      if (ts.isPropertyAccessExpression(node.parent) && node.parent.name === node) {
        const lhsSym = resolveRootSymbol(checker.getSymbolAtLocation(node.parent.expression));
        if (lhsSym) {
          const propMap = propertyCandidatesByObjSymbol.get(lhsSym);
          const propCandidate = propMap?.get(node.text);
          if (propCandidate) {
            attributePropertyCandidate(propCandidate, node);
            return;
          }
        }

        const lhsType = checker.getTypeAtLocation(node.parent.expression);
        const typeSym = lhsType.getSymbol();
        if (!typeSym || !(typeSym.flags & ts.SymbolFlags.Module)) {
          return;
        }
      }

      // Check object destructuring of tracked object properties (`const { listUsers } = userService`)
      if (
        ts.isBindingElement(node.parent) &&
        ts.isObjectBindingPattern(node.parent.parent) &&
        ts.isVariableDeclaration(node.parent.parent.parent) &&
        node.parent.parent.parent.initializer
      ) {
        const propIdent = node.parent.propertyName ?? node.parent.name;
        if (propIdent === node) {
          const rhsSym = resolveRootSymbol(
            checker.getSymbolAtLocation(node.parent.parent.parent.initializer),
          );
          const propCandidate = rhsSym
            ? propertyCandidatesByObjSymbol.get(rhsSym)?.get(node.text)
            : undefined;
          if (propCandidate) {
            attributePropertyCandidate(propCandidate, node);
            return;
          }
        }
      }

      // Skip import specifiers themselves
      if (ts.isImportSpecifier(node.parent) || ts.isImportClause(node.parent)) {
        return;
      }

      // Skip re-export specifiers unless a Next.js entry file re-exports a convention name
      if (ts.isExportSpecifier(node.parent)) {
        const exportedName = node.parent.name.text;
        if (!(isNextEntry && NEXT_CONVENTION_EXPORTS.has(exportedName) && !inTest)) {
          return;
        }
      }

      // Ignore type-position references (`typeof x`) so they cannot launder a value export
      if (isInTypePosition(node)) {
        return;
      }

      const sym = getReferencedSymbol(node);
      if (sym) {
        const candidate = candidatesBySymbol.get(sym);
        if (candidate && node !== candidate.nameNode) {
          const isPropertyAccessLhs =
            ts.isPropertyAccessExpression(node.parent) && node.parent.expression === node;
          const isDestructureRhs =
            ts.isVariableDeclaration(node.parent) &&
            ts.isObjectBindingPattern(node.parent.name) &&
            node.parent.initializer === node;

          if (sf.fileName === candidate.sourceFile.fileName) {
            // Ignore recursive self-references inside the declaration's own body
            const withinOwnDecl =
              node.getStart(sf) >= candidate.declNode.getStart(sf) &&
              node.getEnd() <= candidate.declNode.getEnd();
            if (!withinOwnDecl) {
              candidate.sameFileRefs++;
              recordCallArgs(candidate, node, false);
              if (!isPropertyAccessLhs && !isDestructureRhs) {
                candidate.wholeValueProdRefs++;
              }
            }
          } else if (inTest) {
            candidate.testRefs++;
            candidate.testFiles.add(relFile);
            recordCallArgs(candidate, node, true);
          } else {
            candidate.prodExternalRefs++;
            if (!recordCallArgs(candidate, node, false) && !ts.isExportSpecifier(node.parent)) {
              candidate.nonCallProdRefs++;
            }
            if (!isPropertyAccessLhs && !isDestructureRhs && !ts.isExportSpecifier(node.parent)) {
              candidate.wholeValueProdRefs++;
            }
          }
        }
      }
    }
    ts.forEachChild(node, visit);
  };

  visit(sf);
}

// Propagate whole-value object usage to non-inline-type-literal properties
for (const [objSym, propMap] of propertyCandidatesByObjSymbol.entries()) {
  const parent = candidatesBySymbol.get(objSym);
  if (parent && parent.wholeValueProdRefs > 0) {
    for (const prop of propMap.values()) {
      if (!prop.fromInlineTypeLiteral) {
        prop.prodExternalRefs += parent.wholeValueProdRefs;
      }
    }
  }
}

interface Finding {
  tag: 'LEAKED_PRIVATE_IMPL' | 'TEST_ONLY_ORPHAN' | 'TEST_ONLY_PARAM';
  file: string;
  line: number;
  kind: string;
  name: string;
  sameFileRefs: number;
  testRefs: number;
  testFiles: string[];
}

const uniqueCandidates = new Set<Candidate>([
  ...candidatesBySymbol.values(),
  ...[...propertyCandidatesByObjSymbol.values()].flatMap((m) => [...m.values()]),
]);

const findings: Finding[] = [];

for (const c of uniqueCandidates) {
  if (c.prodExternalRefs === 0 && c.testRefs > 0) {
    findings.push({
      tag: c.sameFileRefs > 0 ? 'LEAKED_PRIVATE_IMPL' : 'TEST_ONLY_ORPHAN',
      file: c.file,
      line: c.line,
      kind: c.kind,
      name: c.name,
      sameFileRefs: c.sameFileRefs,
      testRefs: c.testRefs,
      testFiles: [...c.testFiles].sort(),
    });
    continue;
  }

  // Flag any trailing parameter passed in tests but never in production
  if (
    c.params.length > 0 &&
    c.prodCallCount > 0 &&
    c.nonCallProdRefs === 0 &&
    c.testCallCount > 0 &&
    c.testMaxArgs > c.prodMaxArgs
  ) {
    for (const p of c.params) {
      if (p.index >= c.prodMaxArgs && p.index < c.testMaxArgs) {
        findings.push({
          tag: 'TEST_ONLY_PARAM',
          file: c.file,
          line: p.line,
          kind: 'parameter',
          name: `${c.name}(..., ${p.name})`,
          sameFileRefs: c.sameFileRefs,
          testRefs: c.testRefs,
          testFiles: [...c.testFiles].sort(),
        });
      }
    }
  }
}

findings.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line);

if (jsonMode) {
  console.log(JSON.stringify({ total: findings.length, findings }, null, 2));
} else if (findings.length > 0) {
  const leaked = findings.filter((f) => f.tag === 'LEAKED_PRIVATE_IMPL');
  const orphans = findings.filter((f) => f.tag === 'TEST_ONLY_ORPHAN');
  const params = findings.filter((f) => f.tag === 'TEST_ONLY_PARAM');

  console.error(
    `\n\u2717 Found ${findings.length} test-only export/param violation(s) (${leaked.length} leaked private implementation, ${orphans.length} test-only orphan, ${params.length} test-only parameter):\n`,
  );
  for (const f of findings) {
    console.error(
      `  [${f.tag}] ${f.file}:${f.line} \u2014 ${f.kind} ${f.name} (sameFile=${f.sameFileRefs}, testRefs=${f.testRefs} in ${f.testFiles.join(', ')})`,
    );
  }
  console.error(
    '\nFix: Remove `export` from private helpers, remove test-only override parameters/methods, and test behavior through the public API.\n',
  );
} else {
  console.log('\u2713 No test-only exports found.');
}

process.exit(findings.length === 0 ? 0 : 1);
