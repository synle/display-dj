const path = require('path');
const appPackage = require('./package.json');

const externals = {};
const externalsDeps = [
  'fs',
  'path',
  'electron',
  ...Object.keys(appPackage.optionalDependencies || []),
  ...Object.keys(appPackage.dependencies || [])
];
for(const dep of externalsDeps){
  externals[dep] = `commonjs ${dep}`;
}

console.log('=====================')
console.log('Webpack File: ', __filename);
console.log('App Version: ', appPackage.version)
console.log('App Mode: ', process.env.APP_MODE)
console.log('=====================')

module.exports = {
  entry: ['./src/renderer/index.tsx'],
  devtool: 'source-map',
  output: {
    filename: 'renderer.js',
    path: path.resolve(__dirname, 'build'),
  },
  mode: 'production',
  externals,
  module: {
    rules: [
      {
        test: /\.(js|jsx|ts|tsx)$/,
        use: [
          {
            loader: 'ts-loader',
            options: {
              configFile: 'tsconfig-ui.json',
            },
          },
        ],
        exclude: /node_modules/,
      },
      {
        test: /\.s[ac]ss$/i,
        use: [
          'style-loader', // Creates `style` nodes from JS strings
          'css-loader', // Translates CSS into CommonJS
          'sass-loader', // Compiles Sass to CSS
        ],
      },
      {
        test: /\.svg$/,
        use: ['@svgr/webpack'],
      },
    ],
  },
  resolve: {
    extensions: ['.tsx', '.ts', '.js', '.jsx'],
    alias: {
      src: path.resolve(__dirname, 'src'),
    },
  },
};
