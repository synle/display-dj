const path = require('path');
const webpack = require('webpack');
const appPackage = require('./package.json');

const externals = {};
const externalsDeps = [
  'electron',
  ...Object.keys(appPackage.optionalDependencies || []),
  ...Object.keys(appPackage.dependencies || [])
].filter(s => s !== 'dark-mode');

for(const dep of externalsDeps){
  externals[dep] = `commonjs ${dep}`;
}

console.log('=====================')
console.log('Webpack File: ', __dirname);
console.log('App Version: ', appPackage.version)
console.log('App Mode: ', process.env.APP_MODE)
console.log('=====================')

module.exports = {
  entry: ['./src/main/index.js'],
  output: {
    filename: 'main.js',
    path: path.resolve(__dirname, 'build'),
  },
  mode: 'production',
  target: ['node'],
  externals,
  module: {
    rules: [
      {
        test: /\.tsx?$/,
        use: [
          {
            loader: 'ts-loader',
            options: {
              configFile: 'tsconfig-electron.json',
            },
          },
        ],
        exclude: /node_modules/,
      },
    ],
  },
  resolve: {
    extensions: ['.tsx', '.ts', '.js', '.jsx'],
    alias: {
      src: path.resolve(__dirname, 'src'),
    },
  },
  plugins: [
    new webpack.BannerPlugin({
      banner: [
        `process.env.APP_MODE = "${process.env.APP_MODE}";`,
        `process.env.APP_VERSION = "${appPackage.version}";`,
      ].join(''),
      raw: true,
    }),
  ],
};
