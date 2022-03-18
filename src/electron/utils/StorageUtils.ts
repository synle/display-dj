import * as fs from 'fs';

const StorageUtils = {
  readJSON: (file: string) => {
    try {
      return JSON.parse(fs.readFileSync(file, { encoding: 'utf8', flag: 'r' }).trim());
    } catch (err) {
      return {};
    }
  },
  writeJSON: (file: string, jsonObject: any) => {
    fs.writeFileSync(file, JSON.stringify(jsonObject, null, 2));
  },
};

export default StorageUtils;
