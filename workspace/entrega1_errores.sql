-- Entrega 1 · errores léxicos y sintácticos: uno por línea, a propósito.
-- ▶ ANALIZAR los lista en PROBLEMAS con su línea y columna.
-- Al final de cada línea, el código que debe dar (lo verifica un test).
SELECT FROM cliente;                    -- SYN002 falta la lista del SELECT
CREATE TABLE t (id INT,);               -- SYN001 coma de más antes de )
UPDATE cliente SET nombre 'x';          -- SYN003 falta el =
CREATE TABLA x (id INT);                -- SYN004 TABLA no es una palabra clave
SELECT @ FROM cliente;                  -- LEX001 carácter no reconocido
INSERT INTO t VALUES (1 2);             -- SYN002 falta la coma entre valores
-- La línea siguiente debe dar LEX002 (cadena sin cerrar hasta el final del archivo):
SELECT * FROM cliente WHERE nombre = 'sin cerrar
