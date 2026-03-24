import fs from 'node:fs'
import path from 'node:path'

const version = process.argv[2]?.trim()

if (!version) {
  console.error('Usage: npm run release:version -- <version>')
  process.exit(1)
}

if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error(`Invalid semver version: ${version}`)
  process.exit(1)
}

const rootDir = process.cwd()

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8')
}

const packageJsonPath = path.join(rootDir, 'package.json')
const tauriConfigPath = path.join(rootDir, 'src-tauri', 'tauri.conf.json')
const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml')

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'))
packageJson.version = version
writeJson(packageJsonPath, packageJson)

const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, 'utf8'))
tauriConfig.package.version = version
writeJson(tauriConfigPath, tauriConfig)

const cargoToml = fs.readFileSync(cargoTomlPath, 'utf8')
const cargoVersionPattern = /^version = ".*"$/m
if (!cargoVersionPattern.test(cargoToml)) {
  console.error('Failed to find version in src-tauri/Cargo.toml')
  process.exit(1)
}

const nextCargoToml = cargoToml.replace(
  cargoVersionPattern,
  `version = "${version}"`,
)

fs.writeFileSync(cargoTomlPath, nextCargoToml, 'utf8')

console.log(`Updated app version to ${version}`)
