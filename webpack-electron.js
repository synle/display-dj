const path = require('path');
const webpack = require('webpack');
const appPackage = require('./package.json');

const externals = {};
const externalsDeps = [
  'electron',
  ...Object.keys(appPackage.optionalDependencies || []),
  ...Object.keys(appPackage.dependencies || [])
];
for(const dep of externalsDeps){
  externals[dep] = `commonjs ${dep}`;
}

module.exports = {
  entry: ['./src/main/index.js'],
  output: {
    filename: 'main.js',
    path: path.resolve(__dirname, 'build'),
  },
  mode: 'production',
  target: ['node'],
  externals: externals,
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
        `process.env.APPLICATION_MODE = "${process.env.APPLICATION_MODE}";`,
        `process.env.APP_VERSION = "${appPackage.version}";`,
      ].join(''),
      raw: true,
    }),
  ],
};
