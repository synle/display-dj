const path = require('path');

module.exports = {
  entry: ['regenerator-runtime/runtime.js', './src/renderer/index.jsx'],
  devtool: 'source-map',
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
        use: {
          loader: 'babel-loader',
          options: {
            plugins: [
              [
                '@babel/plugin-transform-react-jsx',
                {
                  runtime: 'automatic',
                },
              ],
            ],
          },
        },
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
    ],
  },
  resolve: {
    extensions: ['*', '.js', '.jsx'],
  },
};
