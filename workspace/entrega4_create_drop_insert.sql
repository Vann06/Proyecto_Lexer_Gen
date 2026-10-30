-- ═══════════════════════════════════════════════════════════════════════
-- Entrega 4 · CREATE, DROP e INSERT
-- ▶ EJECUTAR → RESULTADOS (cada sentencia) y CATÁLOGO (tablas creadas).
-- Se puede ejecutar las veces que se quiera: empieza con su base limpia.
-- ═══════════════════════════════════════════════════════════════════════
DROP DATABASE IF EXISTS entrega4;
CREATE DATABASE entrega4;
USE entrega4;

-- Los cinco tipos del enunciado y las restricciones que se guardan en
-- metadata.txt: PRIMARY KEY, NOT NULL, UNIQUE, FOREIGN KEY y CHECK.
CREATE TABLE estudiantes (
    id       INT PRIMARY KEY,
    nombre   VARCHAR(40) NOT NULL,
    carne    VARCHAR(10) UNIQUE,
    edad     INT,
    promedio FLOAT,
    activo   BOOLEAN,
    ingreso  DATE,
    CHECK (edad >= 0)
);

CREATE TABLE cursos (
    id       INT PRIMARY KEY,
    nombre   VARCHAR(30) NOT NULL,
    creditos INT
);

-- Llave primaria compuesta y llaves foráneas.
CREATE TABLE inscripciones (
    estudiante_id INT,
    curso_id      INT,
    nota          FLOAT,
    CONSTRAINT pk_insc PRIMARY KEY (estudiante_id, curso_id),
    FOREIGN KEY (estudiante_id) REFERENCES estudiantes (id),
    FOREIGN KEY (curso_id) REFERENCES cursos
);

-- IF NOT EXISTS: no falla aunque la tabla ya exista.
CREATE TABLE IF NOT EXISTS cursos (x INT);

-- INSERT con todas las columnas, varias filas a la vez, NULL, una cadena
-- con comilla y coma, y una fecha bisiesta.
INSERT INTO estudiantes VALUES
    (1, 'Ana',            'A001', 20, 88.5, TRUE,  '2023-01-15'),
    (2, 'Luis',           'A002', 17, 75.0, TRUE,  '2024-01-10'),
    (3, 'O''Brien, José', NULL,   22, NULL, FALSE, '2024-02-29');

-- INSERT con lista de columnas: las omitidas quedan en NULL.
INSERT INTO estudiantes (id, nombre) VALUES (4, 'Sofía');

-- Un INT en una columna FLOAT se convierte (80 -> 80.0).
INSERT INTO cursos VALUES (1, 'Compiladores', 5), (2, 'Bases de datos', 4), (3, 'Redes', 3);
INSERT INTO inscripciones VALUES (1, 1, 91), (1, 3, 55.5), (2, 3, 80), (3, 2, 70);

SELECT * FROM estudiantes;
SELECT * FROM inscripciones;

-- DROP TABLE y DROP TABLE IF EXISTS (no falla si ya no existe).
CREATE TABLE temporal (n INT);
DROP TABLE temporal;
DROP TABLE IF EXISTS temporal;

SHOW TABLES;
DESCRIBE estudiantes;
