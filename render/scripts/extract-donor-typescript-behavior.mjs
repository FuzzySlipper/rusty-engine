import { execFileSync } from 'node:child_process';

import ts from 'typescript';

const [donor, pin, donorPath] = process.argv.slice(2);
if (!donor || !pin || !donorPath) {
  console.error('usage: node extract-donor-typescript-behavior.mjs DONOR PIN PATH');
  process.exit(2);
}

const sourceText = execFileSync(
  'git',
  ['-C', donor, 'show', `${pin}:${donorPath}`],
  { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
);
const source = ts.createSourceFile(
  donorPath,
  sourceText,
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TS,
);
const records = [];

function lineOf(node) {
  return source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
}

function hasModifier(node, kind) {
  return ts.canHaveModifiers(node)
    && (ts.getModifiers(node) ?? []).some((modifier) => modifier.kind === kind);
}

function exported(node) {
  return hasModifier(node, ts.SyntaxKind.ExportKeyword)
    || hasModifier(node, ts.SyntaxKind.DefaultKeyword);
}

function publicMember(node) {
  return !(node.name && ts.isPrivateIdentifier(node.name))
    && !hasModifier(node, ts.SyntaxKind.PrivateKeyword)
    && !hasModifier(node, ts.SyntaxKind.ProtectedKeyword);
}

function nameOf(name, fallback) {
  if (!name) return fallback;
  if (ts.isIdentifier(name) || ts.isPrivateIdentifier(name)) return name.text;
  if (ts.isStringLiteral(name) || ts.isNumericLiteral(name)) return name.text;
  return name.getText(source);
}

function slug(value) {
  return value
    .normalize('NFKD')
    .replace(/[^A-Za-z0-9_.-]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 160) || 'unnamed';
}

function add(kind, node, symbol) {
  const line = lineOf(node);
  records.push({
    itemId: `${kind}:${donorPath}:${line}:${slug(symbol)}`,
    kind,
    line,
    symbol,
  });
}

function declarationKind(statement) {
  if (ts.isClassDeclaration(statement)) return 'class';
  if (ts.isInterfaceDeclaration(statement)) return 'interface';
  if (ts.isTypeAliasDeclaration(statement)) return 'type';
  if (ts.isEnumDeclaration(statement)) return 'enum';
  if (ts.isFunctionDeclaration(statement)) return 'function';
  if (ts.isModuleDeclaration(statement)) return 'namespace';
  return 'declaration';
}

for (const statement of source.statements) {
  if (ts.isExportDeclaration(statement)) {
    const moduleName = statement.moduleSpecifier && ts.isStringLiteral(statement.moduleSpecifier)
      ? statement.moduleSpecifier.text
      : 'local';
    if (statement.exportClause && ts.isNamedExports(statement.exportClause)) {
      for (const element of statement.exportClause.elements) {
        add('api', element, `reexport:${element.name.text}`);
      }
    } else {
      add('api', statement, `reexport-all:${moduleName}`);
    }
    continue;
  }

  if (!exported(statement)) continue;

  if (ts.isVariableStatement(statement)) {
    for (const declaration of statement.declarationList.declarations) {
      add('api', declaration, `const:${nameOf(declaration.name, 'default')}`);
    }
    continue;
  }

  const declarationName = nameOf(statement.name, 'default');
  add('api', statement, `${declarationKind(statement)}:${declarationName}`);

  if (ts.isClassDeclaration(statement)) {
    for (const member of statement.members) {
      if (!publicMember(member)) continue;
      if (ts.isConstructorDeclaration(member)) {
        add('api', member, `method:${declarationName}.constructor`);
      } else if (ts.isMethodDeclaration(member)) {
        add('api', member, `method:${declarationName}.${nameOf(member.name, 'unnamed')}`);
      } else if (ts.isGetAccessorDeclaration(member)) {
        add('api', member, `getter:${declarationName}.${nameOf(member.name, 'unnamed')}`);
      } else if (ts.isSetAccessorDeclaration(member)) {
        add('api', member, `setter:${declarationName}.${nameOf(member.name, 'unnamed')}`);
      } else if (ts.isPropertyDeclaration(member)) {
        add('api', member, `property:${declarationName}.${nameOf(member.name, 'unnamed')}`);
      }
    }
  }
}

function visit(node) {
  if (ts.isCallExpression(node)) {
    const callee = node.expression;
    const testCall = ts.isIdentifier(callee) && (callee.text === 'test' || callee.text === 'it');
    const qualifiedTestCall = ts.isPropertyAccessExpression(callee)
      && ts.isIdentifier(callee.expression)
      && (callee.expression.text === 'test' || callee.expression.text === 'it');
    const title = node.arguments[0];
    if ((testCall || qualifiedTestCall) && title && ts.isStringLiteralLike(title)) {
      add('test', node, title.text.replace(/\s+/g, ' ').trim());
    }
  }
  ts.forEachChild(node, visit);
}
visit(source);

if (records.length === 0) {
  records.push({
    itemId: `internal:${donorPath}`,
    kind: 'internal',
    line: 0,
    symbol: 'no-exported-api-or-test',
  });
}

records.sort((left, right) => left.line - right.line
  || left.kind.localeCompare(right.kind)
  || left.symbol.localeCompare(right.symbol));

const seen = new Set();
for (const record of records) {
  if (seen.has(record.itemId)) {
    throw new Error(`duplicate donor behavior item: ${record.itemId}`);
  }
  seen.add(record.itemId);
  process.stdout.write(
    `${record.itemId}\t${record.kind}\t${donorPath}\t${record.line}\t${record.symbol}\n`,
  );
}
