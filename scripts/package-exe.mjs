#!/usr/bin/env node
import { execSync } from 'child_process';
import { existsSync, mkdirSync, copyFileSync, readFileSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { rcedit } from 'rcedit';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const rootDir = join(__dirname, '..');
const requiredIconSizes = [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 128, 256];

function readPeIconResources(exePath) {
  const buffer = readFileSync(exePath);

  const u16 = offset => buffer.readUInt16LE(offset);
  const u32 = offset => buffer.readUInt32LE(offset);
  const peOffset = u32(0x3c);
  if (buffer.toString('ascii', peOffset, peOffset + 4) !== 'PE\0\0') {
    throw new Error(`${exePath} is not a PE executable`);
  }

  const sectionCount = u16(peOffset + 6);
  const optionalHeaderSize = u16(peOffset + 20);
  const optionalHeaderOffset = peOffset + 24;
  const optionalHeaderMagic = u16(optionalHeaderOffset);
  const dataDirectoryOffset = optionalHeaderOffset + (optionalHeaderMagic === 0x20b ? 112 : 96);
  const resourceRva = u32(dataDirectoryOffset + 16);
  if (!resourceRva) {
    return [];
  }

  const sectionTableOffset = optionalHeaderOffset + optionalHeaderSize;
  const sections = [];
  for (let sectionIndex = 0; sectionIndex < sectionCount; sectionIndex += 1) {
    const offset = sectionTableOffset + sectionIndex * 40;
    sections.push({
      virtualSize: u32(offset + 8),
      virtualAddress: u32(offset + 12),
      rawSize: u32(offset + 16),
      rawPointer: u32(offset + 20)
    });
  }

  function rvaToOffset(rva) {
    const section = sections.find(item => {
      const sectionSize = Math.max(item.virtualSize, item.rawSize);
      return rva >= item.virtualAddress && rva < item.virtualAddress + sectionSize;
    });

    if (!section) {
      throw new Error(`Could not map resource RVA 0x${rva.toString(16)}`);
    }

    return section.rawPointer + (rva - section.virtualAddress);
  }

  const resourceBaseOffset = rvaToOffset(resourceRva);

  function resourceDirectoryEntries(directoryOffset) {
    const entryCount = u16(directoryOffset + 12) + u16(directoryOffset + 14);
    const entries = [];
    for (let entryIndex = 0; entryIndex < entryCount; entryIndex += 1) {
      const offset = directoryOffset + 16 + entryIndex * 8;
      const nameRaw = u32(offset);
      const dataRaw = u32(offset + 4);
      entries.push({
        id: nameRaw & 0xffff,
        isDirectory: (dataRaw & 0x80000000) !== 0,
        offset: dataRaw & 0x7fffffff
      });
    }
    return entries;
  }

  function dataEntriesForType(typeId) {
    const typeEntry = resourceDirectoryEntries(resourceBaseOffset)
      .find(entry => entry.id === typeId && entry.isDirectory);
    if (!typeEntry) {
      return [];
    }

    const result = [];
    const nameEntries = resourceDirectoryEntries(resourceBaseOffset + typeEntry.offset);
    for (const nameEntry of nameEntries) {
      if (!nameEntry.isDirectory) {
        continue;
      }

      const languageEntries = resourceDirectoryEntries(resourceBaseOffset + nameEntry.offset);
      for (const languageEntry of languageEntries) {
        if (languageEntry.isDirectory) {
          continue;
        }

        const dataEntryOffset = resourceBaseOffset + languageEntry.offset;
        result.push({
          id: nameEntry.id,
          languageId: languageEntry.id,
          dataOffset: rvaToOffset(u32(dataEntryOffset)),
          size: u32(dataEntryOffset + 4)
        });
      }
    }
    return result;
  }

  const iconResourcesById = new Map(dataEntriesForType(3).map(icon => [icon.id, icon]));
  const groupIconResources = dataEntriesForType(14);

  return groupIconResources.map(group => {
    const data = buffer.subarray(group.dataOffset, group.dataOffset + group.size);
    const count = data.readUInt16LE(4);
    const entries = [];
    for (let entryIndex = 0; entryIndex < count; entryIndex += 1) {
      const offset = 6 + entryIndex * 14;
      const iconId = data.readUInt16LE(offset + 12);
      const iconResource = iconResourcesById.get(iconId);
      entries.push({
        width: data[offset] || 256,
        height: data[offset + 1] || 256,
        bitDepth: data.readUInt16LE(offset + 6),
        bytesInGroup: data.readUInt32LE(offset + 8),
        iconId,
        resourceBytes: iconResource?.size ?? 0,
        signature: iconResource
          ? buffer.subarray(iconResource.dataOffset, iconResource.dataOffset + 8).toString('hex')
          : 'missing'
      });
    }
    return { id: group.id, languageId: group.languageId, entries };
  });
}

function verifyExeIconResources(exePath) {
  const groups = readPeIconResources(exePath);
  const appIcon = groups.find(group => group.id === 32512) ?? groups[0];
  if (!appIcon) {
    throw new Error(`No icon group found in ${exePath}`);
  }

  const embeddedSizes = appIcon.entries.map(entry => entry.width).sort((a, b) => a - b);
  const missingSizes = requiredIconSizes.filter(size => !embeddedSizes.includes(size));
  if (missingSizes.length > 0) {
    throw new Error(`AgentWatcher.exe icon resource is missing sizes: ${missingSizes.join(', ')}`);
  }

  console.log(`Verified exe icon resources: ${embeddedSizes.join(', ')}px`);
}

console.log('Building AgentWatcher...');

try {
  execSync('npm run generate:icons', { cwd: rootDir, stdio: 'inherit' });

  const releaseDir = join(rootDir, 'src-tauri', 'target', 'release');
  const exePath = join(releaseDir, 'agentwatcher.exe');
  if (existsSync(exePath)) {
    rmSync(exePath, { force: true });
  }

  // Run Tauri build
  execSync('npm run build', { cwd: rootDir, stdio: 'inherit' });

  console.log('\nPackaging executable...');

  // Locate the built executable
  if (!existsSync(exePath)) {
    throw new Error(`Built executable not found at ${exePath}`);
  }

  // Create output directory
  const outputDir = join(rootDir, 'artifacts', 'AgentWatcher');
  if (!existsSync(outputDir)) {
    mkdirSync(outputDir, { recursive: true });
  }

  const outputExe = join(outputDir, 'AgentWatcher.exe');
  const stagedExe = join(tmpdir(), `AgentWatcher-${process.pid}.staged.exe`);
  rmSync(stagedExe, { force: true });

  const iconPath = join(rootDir, 'src-tauri', 'icons', 'icon.ico');
  copyFileSync(exePath, stagedExe);
  await rcedit(stagedExe, { icon: iconPath });
  verifyExeIconResources(stagedExe);
  rmSync(outputExe, { force: true });
  copyFileSync(stagedExe, outputExe);
  rmSync(stagedExe, { force: true });
  console.log(`Wrote executable with embedded icon to ${outputExe}`);

  // Copy bridge extension folder
  const bridgeSourceDir = join(rootDir, 'vscode-agentwatcher-bridge');
  const bridgeOutputDir = join(outputDir, 'vscode-agentwatcher-bridge');

  if (existsSync(bridgeSourceDir)) {
    if (!existsSync(bridgeOutputDir)) {
      mkdirSync(bridgeOutputDir, { recursive: true });
    }

    // Copy essential bridge files
    const bridgeFiles = ['extension.js', 'package.json', 'README.md'];
    bridgeFiles.forEach(file => {
      const sourcePath = join(bridgeSourceDir, file);
      const destPath = join(bridgeOutputDir, file);
      if (existsSync(sourcePath)) {
        copyFileSync(sourcePath, destPath);
      }
    });

    const bridgePackage = JSON.parse(readFileSync(join(bridgeSourceDir, 'package.json'), 'utf8'));
    const bridgeVersion = String(bridgePackage.version || '0.0.0').replace(/[^a-zA-Z0-9._-]/g, '_');
    const bridgeVsix = join(bridgeOutputDir, `agentwatcher-bridge-${bridgeVersion}.vsix`);
    execSync(`npx --yes @vscode/vsce package --skip-license --out "${bridgeVsix}"`, {
      cwd: bridgeSourceDir,
      stdio: 'inherit'
    });

    console.log(`Copied bridge extension to ${bridgeOutputDir}`);
  }

  console.log('\nPackaging complete.');
  console.log(`Output: ${outputDir}`);
  console.log(`   - AgentWatcher.exe`);
  console.log(`   - vscode-agentwatcher-bridge/`);
  console.log(`   - vscode-agentwatcher-bridge/*.vsix`);

} catch (error) {
  console.error('Packaging failed:', error.message);
  process.exit(1);
}
