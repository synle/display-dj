const path = require('path');

module.exports = {
  entry: ['./src/renderer/index.tsx'],
  devtool: 'source-map',
  output: {
    filename: 'renderer.js',
    path: path.resolve(__dirname, 'build'),
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
          // Creates `style` nodes from JS strings
          'style-loader',
          // Translates CSS into CommonJS
          'css-loader',
          // Compiles Sass to CSS
          'sass-loader',
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
