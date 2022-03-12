declare const StorageUtils: {
    readJSON: (file: string) => any;
    writeJSON: (file: string, jsonObject: Record<string, any>) => void;
};
export default StorageUtils;
