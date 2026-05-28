#!/usr/bin/env node
import { writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deflateSync } from 'node:zlib';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '..');
const outputIco = resolve(repoRoot, 'src-tauri', 'icons', 'icon.ico');
const RUNTIME_ICON_SIZES = [256];
const ICO_SIZES = [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 128, 256];
const SUPERSAMPLE = 4;
const BASE_SIZE = 256;

const COLORS = {
  transparent: [0, 0, 0, 0],
  backgroundTop: [32, 32, 32, 255],
  backgroundBottom: [26, 26, 26, 255],
  border: [55, 148, 255, 255],
  letter: [255, 255, 255, 255]
};

function makeCanvas(width, height) {
  return {
    width,
    height,
    data: new Uint8ClampedArray(width * height * 4)
  };
}

function pixelOffset(canvas, x, y) {
  return (y * canvas.width + x) * 4;
}

function setPixel(canvas, x, y, color) {
  if (x < 0 || y < 0 || x >= canvas.width || y >= canvas.height) return;
  const offset = pixelOffset(canvas, x, y);
  canvas.data[offset] = color[0];
  canvas.data[offset + 1] = color[1];
  canvas.data[offset + 2] = color[2];
  canvas.data[offset + 3] = color[3];
}

function blendColor(top, bottom, t) {
  return [
    Math.round(top[0] + (bottom[0] - top[0]) * t),
    Math.round(top[1] + (bottom[1] - top[1]) * t),
    Math.round(top[2] + (bottom[2] - top[2]) * t),
    Math.round(top[3] + (bottom[3] - top[3]) * t)
  ];
}

function insideRoundedRect(px, py, x, y, width, height, radius) {
  const innerLeft = x + radius;
  const innerRight = x + width - radius;
  const innerTop = y + radius;
  const innerBottom = y + height - radius;

  if (px >= innerLeft && px <= innerRight && py >= y && py <= y + height) return true;
  if (py >= innerTop && py <= innerBottom && px >= x && px <= x + width) return true;

  const cx = px < innerLeft ? innerLeft : innerRight;
  const cy = py < innerTop ? innerTop : innerBottom;
  const dx = px - cx;
  const dy = py - cy;
  return dx * dx + dy * dy <= radius * radius;
}

function fillRoundedRect(canvas, x, y, width, height, radius, colorAt) {
  const left = Math.max(0, Math.floor(x));
  const right = Math.min(canvas.width - 1, Math.ceil(x + width));
  const top = Math.max(0, Math.floor(y));
  const bottom = Math.min(canvas.height - 1, Math.ceil(y + height));

  for (let py = top; py <= bottom; py += 1) {
    for (let px = left; px <= right; px += 1) {
      if (!insideRoundedRect(px + 0.5, py + 0.5, x, y, width, height, radius)) continue;
      setPixel(canvas, px, py, colorAt(px, py));
    }
  }
}

function strokeRoundedRect(canvas, x, y, width, height, radius, strokeWidth, color) {
  const innerX = x + strokeWidth;
  const innerY = y + strokeWidth;
  const innerWidth = width - strokeWidth * 2;
  const innerHeight = height - strokeWidth * 2;
  const innerRadius = Math.max(0, radius - strokeWidth);
  const left = Math.max(0, Math.floor(x));
  const right = Math.min(canvas.width - 1, Math.ceil(x + width));
  const top = Math.max(0, Math.floor(y));
  const bottom = Math.min(canvas.height - 1, Math.ceil(y + height));

  for (let py = top; py <= bottom; py += 1) {
    for (let px = left; px <= right; px += 1) {
      const cx = px + 0.5;
      const cy = py + 0.5;
      const insideOuter = insideRoundedRect(cx, cy, x, y, width, height, radius);
      const insideInner = insideRoundedRect(cx, cy, innerX, innerY, innerWidth, innerHeight, innerRadius);
      if (insideOuter && !insideInner) setPixel(canvas, px, py, color);
    }
  }
}

function fillRect(canvas, x, y, width, height, color) {
  const left = Math.max(0, Math.floor(x));
  const right = Math.min(canvas.width, Math.ceil(x + width));
  const top = Math.max(0, Math.floor(y));
  const bottom = Math.min(canvas.height, Math.ceil(y + height));

  for (let py = top; py < bottom; py += 1) {
    for (let px = left; px < right; px += 1) {
      setPixel(canvas, px, py, color);
    }
  }
}

function distanceToSegment(px, py, x1, y1, x2, y2) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const lengthSquared = dx * dx + dy * dy;
  if (lengthSquared === 0) return Math.hypot(px - x1, py - y1);
  const t = Math.max(0, Math.min(1, ((px - x1) * dx + (py - y1) * dy) / lengthSquared));
  const sx = x1 + t * dx;
  const sy = y1 + t * dy;
  return Math.hypot(px - sx, py - sy);
}

function drawLine(canvas, x1, y1, x2, y2, width, color) {
  const radius = width / 2;
  const left = Math.max(0, Math.floor(Math.min(x1, x2) - radius));
  const right = Math.min(canvas.width - 1, Math.ceil(Math.max(x1, x2) + radius));
  const top = Math.max(0, Math.floor(Math.min(y1, y2) - radius));
  const bottom = Math.min(canvas.height - 1, Math.ceil(Math.max(y1, y2) + radius));

  for (let py = top; py <= bottom; py += 1) {
    for (let px = left; px <= right; px += 1) {
      if (distanceToSegment(px + 0.5, py + 0.5, x1, y1, x2, y2) <= radius) {
        setPixel(canvas, px, py, color);
      }
    }
  }
}

function drawAw(canvas, scale) {
  const w = 17 * scale;
  const c = COLORS.letter;

  drawLine(canvas, 48 * scale, 166 * scale, 70 * scale, 92 * scale, w, c);
  drawLine(canvas, 70 * scale, 92 * scale, 96 * scale, 166 * scale, w, c);
  drawLine(canvas, 56 * scale, 132 * scale, 87 * scale, 132 * scale, 14 * scale, c);

  drawLine(canvas, 112 * scale, 94 * scale, 125 * scale, 166 * scale, w, c);
  drawLine(canvas, 125 * scale, 166 * scale, 143 * scale, 116 * scale, w, c);
  drawLine(canvas, 143 * scale, 116 * scale, 161 * scale, 166 * scale, w, c);
  drawLine(canvas, 161 * scale, 166 * scale, 178 * scale, 94 * scale, w, c);
}

const SMALL_GLYPHS = {
  A: [
    '0110',
    '1001',
    '1001',
    '1111',
    '1001',
    '1001',
    '1001'
  ],
  W: [
    '10001',
    '10001',
    '10001',
    '10101',
    '10101',
    '10101',
    '01010'
  ]
};

function drawGlyph(canvas, glyph, x, y, scale, color) {
  glyph.forEach((row, rowIndex) => {
    for (let columnIndex = 0; columnIndex < row.length; columnIndex += 1) {
      if (row[columnIndex] === '1') {
        fillRect(canvas, x + columnIndex * scale, y + rowIndex * scale, scale, scale, color);
      }
    }
  });
}

function drawSmallAw(canvas, size) {
  let glyphScale = 1;
  if (size >= 40) glyphScale = 3;
  else if (size >= 24) glyphScale = 2;

  const gap = glyphScale;
  const glyphWidth = (SMALL_GLYPHS.A[0].length + SMALL_GLYPHS.W[0].length) * glyphScale + gap;
  const glyphHeight = SMALL_GLYPHS.A.length * glyphScale;
  const x = Math.round((size - glyphWidth) / 2);
  const y = Math.round((size - glyphHeight) / 2) + (size <= 20 ? 1 : 0);

  drawGlyph(canvas, SMALL_GLYPHS.A, x, y, glyphScale, COLORS.letter);
  drawGlyph(canvas, SMALL_GLYPHS.W, x + SMALL_GLYPHS.A[0].length * glyphScale + gap, y, glyphScale, COLORS.letter);
}

function renderSmallIcon(size) {
  const canvas = makeCanvas(size, size);
  canvas.data.fill(0);

  const inset = size <= 20 ? 1 : 2;
  const rectSize = size - inset * 2;
  const radius = Math.max(2, Math.round(size * 0.18));
  const borderWidth = size >= 32 ? 2 : 1;

  fillRoundedRect(canvas, inset, inset, rectSize, rectSize, radius, (_px, py) => {
    const t = (py - inset) / rectSize;
    return blendColor(COLORS.backgroundTop, COLORS.backgroundBottom, Math.max(0, Math.min(1, t)));
  });
  strokeRoundedRect(canvas, inset, inset, rectSize, rectSize, radius, borderWidth, COLORS.border);
  drawSmallAw(canvas, size);

  return canvas;
}

function renderIcon(size) {
  if (size <= 48) {
    return renderSmallIcon(size);
  }

  const highSize = size * SUPERSAMPLE;
  const scale = highSize / BASE_SIZE;
  const canvas = makeCanvas(highSize, highSize);
  canvas.data.fill(0);

  const x = 8 * scale;
  const y = 8 * scale;
  const rectSize = 240 * scale;
  const radius = 48 * scale;
  const border = Math.max(1, 6 * scale);

  fillRoundedRect(canvas, x, y, rectSize, rectSize, radius, (_px, py) => {
    const t = (py - y) / rectSize;
    return blendColor(COLORS.backgroundTop, COLORS.backgroundBottom, Math.max(0, Math.min(1, t)));
  });
  strokeRoundedRect(canvas, x, y, rectSize, rectSize, radius, border, COLORS.border);
  drawAw(canvas, scale);

  return downsample(canvas, size);
}

function downsample(source, size) {
  const dest = makeCanvas(size, size);
  const factor = SUPERSAMPLE;

  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      let r = 0;
      let g = 0;
      let b = 0;
      let a = 0;
      for (let sy = 0; sy < factor; sy += 1) {
        for (let sx = 0; sx < factor; sx += 1) {
          const offset = pixelOffset(source, x * factor + sx, y * factor + sy);
          r += source.data[offset];
          g += source.data[offset + 1];
          b += source.data[offset + 2];
          a += source.data[offset + 3];
        }
      }
      const samples = factor * factor;
      setPixel(dest, x, y, [
        Math.round(r / samples),
        Math.round(g / samples),
        Math.round(b / samples),
        Math.round(a / samples)
      ]);
    }
  }

  return dest;
}

function iconImageData(canvas) {
  const headerSize = 40;
  const pixelDataSize = canvas.width * canvas.height * 4;
  const maskStride = Math.ceil(canvas.width / 32) * 4;
  const maskSize = maskStride * canvas.height;
  const buffer = Buffer.alloc(headerSize + pixelDataSize + maskSize);

  buffer.writeUInt32LE(headerSize, 0);
  buffer.writeInt32LE(canvas.width, 4);
  buffer.writeInt32LE(canvas.height * 2, 8);
  buffer.writeUInt16LE(1, 12);
  buffer.writeUInt16LE(32, 14);
  buffer.writeUInt32LE(0, 16);
  buffer.writeUInt32LE(pixelDataSize, 20);
  buffer.writeInt32LE(0, 24);
  buffer.writeInt32LE(0, 28);
  buffer.writeUInt32LE(0, 32);
  buffer.writeUInt32LE(0, 36);

  let writeOffset = headerSize;
  for (let y = canvas.height - 1; y >= 0; y -= 1) {
    for (let x = 0; x < canvas.width; x += 1) {
      const readOffset = pixelOffset(canvas, x, y);
      buffer[writeOffset] = canvas.data[readOffset + 2];
      buffer[writeOffset + 1] = canvas.data[readOffset + 1];
      buffer[writeOffset + 2] = canvas.data[readOffset];
      buffer[writeOffset + 3] = canvas.data[readOffset + 3];
      writeOffset += 4;
    }
  }

  return buffer;
}

const CRC_TABLE = new Uint32Array(256);
for (let index = 0; index < 256; index += 1) {
  let value = index;
  for (let bit = 0; bit < 8; bit += 1) {
    value = (value & 1) ? (0xedb88320 ^ (value >>> 1)) : (value >>> 1);
  }
  CRC_TABLE[index] = value >>> 0;
}

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const typeBuffer = Buffer.from(type, 'ascii');
  const chunk = Buffer.alloc(12 + data.length);
  chunk.writeUInt32BE(data.length, 0);
  typeBuffer.copy(chunk, 4);
  data.copy(chunk, 8);
  const crc = crc32(Buffer.concat([typeBuffer, data]));
  chunk.writeUInt32BE(crc, 8 + data.length);
  return chunk;
}

function pngImageData(canvas) {
  const header = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(canvas.width, 0);
  ihdr.writeUInt32BE(canvas.height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  const raw = Buffer.alloc((canvas.width * 4 + 1) * canvas.height);
  let writeOffset = 0;
  for (let y = 0; y < canvas.height; y += 1) {
    raw[writeOffset] = 0;
    writeOffset += 1;
    for (let x = 0; x < canvas.width; x += 1) {
      const readOffset = pixelOffset(canvas, x, y);
      raw[writeOffset] = canvas.data[readOffset];
      raw[writeOffset + 1] = canvas.data[readOffset + 1];
      raw[writeOffset + 2] = canvas.data[readOffset + 2];
      raw[writeOffset + 3] = canvas.data[readOffset + 3];
      writeOffset += 4;
    }
  }

  return Buffer.concat([
    header,
    pngChunk('IHDR', ihdr),
    pngChunk('IDAT', deflateSync(raw, { level: 9 })),
    pngChunk('IEND', Buffer.alloc(0))
  ]);
}

function writeIco(images) {
  const headerSize = 6;
  const entrySize = 16;
  const header = Buffer.alloc(headerSize + images.length * entrySize);
  const imageBuffers = images.map(({ size, canvas }) => (size === 256 ? pngImageData(canvas) : iconImageData(canvas)));
  let imageOffset = header.length;

  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(images.length, 4);

  images.forEach(({ size }, index) => {
    const entryOffset = headerSize + index * entrySize;
    const imageBuffer = imageBuffers[index];
    header[entryOffset] = size >= 256 ? 0 : size;
    header[entryOffset + 1] = size >= 256 ? 0 : size;
    header[entryOffset + 2] = 0;
    header[entryOffset + 3] = 0;
    header.writeUInt16LE(1, entryOffset + 4);
    header.writeUInt16LE(32, entryOffset + 6);
    header.writeUInt32LE(imageBuffer.length, entryOffset + 8);
    header.writeUInt32LE(imageOffset, entryOffset + 12);
    imageOffset += imageBuffer.length;
  });

  writeFileSync(outputIco, Buffer.concat([header, ...imageBuffers]));
}

function writeRuntimeIcons(images) {
  for (const runtimeSize of RUNTIME_ICON_SIZES) {
    const runtimeImage = images.find(({ size }) => size === runtimeSize);
    if (!runtimeImage) {
      throw new Error(`Missing ${runtimeSize}x${runtimeSize} runtime icon image`);
    }

    const outputRuntimeIcon = resolve(repoRoot, 'src-tauri', 'icons', `icon-runtime-${runtimeSize}.rgba`);
    writeFileSync(outputRuntimeIcon, Buffer.from(runtimeImage.canvas.data));
    console.log(`Generated ${outputRuntimeIcon}`);
  }
}

const images = ICO_SIZES.map((size) => {
  console.log(`Rendering ${size}x${size}`);
  return { size, canvas: renderIcon(size) };
});

writeIco(images);
writeRuntimeIcons(images);
console.log(`Generated ${outputIco}`);
