import fs from 'fs';

const StorageUtils = {
  readJSON: (file) => {
    try {
      return JSON.parse(fs.readFileSync(file, { encoding: 'utf8', flag: 'r' }).trim());
    } catch (err) {
      return {};
    }
  },
  writeJSON: (file, jsonObject) => {
    fs.writeFileSync(file, JSON.stringify(jsonObject, null, 2));
  },
};

export default StorageUtils;
