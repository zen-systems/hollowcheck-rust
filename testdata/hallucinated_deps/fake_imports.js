// Test file with hallucinated JavaScript dependencies

import fs from 'fs';  // builtin - should be ignored
import path from 'path';  // builtin - should be ignored

import express from 'express';  // real package - should pass
import lodash from 'lodash';  // real package - should pass

// These are fake packages that should be flagged
import nonexistentAiGeneratedPackage from 'nonexistent-ai-generated-package-12345';
import { helper } from 'fake-utils-library-xyz';
const madeUp = require('totally-made-up-sdk');

function main() {
    console.log("Hello, world!");
}

main();
