const path = require('path');

module.exports = {
  entry: ["regenerator-runtime/runtime.js", './src/renderer/index.jsx'],
  output: {
    filename: 'renderer.js',
    path: path.resolve(__dirname, 'dist'),
  },
  mode: 'production',
  externals: {
    fs: 'commonjs fs',
    path: 'commonjs path',
    electron: 'commonjs electron',
    react: 'commonjs react',
    'react-dom': 'commonjs react-dom',
  },
  module: {
    rules: [
      {
        test: /\.(js|jsx)$/,
        exclude: /node_modules/,
        use: ['babel-loader'],
      },
    ],
  },
  resolve: {
    extensions: ['*', '.js', '.jsx'],
  },
};

