const path = require('path');

module.exports = {
  entry: ['./src/electron/index.js'],
  output: {
    filename: 'main.js',
    path: path.resolve(__dirname, 'build'),
  },
  mode: 'production',
  target: ['node'],
  externals: {
    electron: 'commonjs electron',
    'react-router-dom': 'commonjs react-router-dom',
    '@hensm/ddcci': 'commonjs @hensm/ddcci',
  },
  module: {
    rules: [
      {
        test: /\.tsx?$/,
        use: 'ts-loader',
        exclude: /node_modules/,
      }
    ],
  },
  resolve: {
    extensions: ['.tsx', '.ts', '.js', '.jsx'],
    alias: {
      src: path.resolve(__dirname, 'src'),
    },
  },
};
